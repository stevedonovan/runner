//! A Rust snippet runner.
//!
//! Please see [readme](https://github.com/stevedonovan/runner/blob/master/readme.md)
extern crate easy_shortcuts as es;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate serde_derive;
use lapp;
use shlex;

use es::traits::*;
use std::process;
use std::env;
use std::fs;
use std::path::{Path,PathBuf};
use std::collections::{HashSet};
use std::env::consts::EXE_SUFFIX;

mod crate_utils;
mod platform;
mod strutil;
mod meta;
mod cargo_lock;
mod cache;
mod state;
mod compile;

use platform::{open,edit};
use crate_utils::RUSTUP_LIB;
use state::State;
use compile::{compile_crate,massage_snippet};

const VERSION: &str = "0.4.0";

const USAGE: &str = "
Compile and run small Rust snippets
  -s, --static build statically (default is dynamic)
  -d, --dynamic overrides --static in env.rs
  -O, --optimize optimized static build
  -e, --expression evaluate an expression
  -i, --iterator iterate over an expression
  -n, --lines evaluate expression over stdin; the var 'line' is defined
  -x, --extern... (string) add an extern crate to the snippet
  -X, --wild... (string) like -x but implies wildcard import
  -M, --macro... (string) like -x but implies macro import
  -p, --prepend (default '') put this statement in body (useful for -i etc)
  -N, --no-prelude do not include runner prelude
  -c, --compile-only  compiles program and copies to output dir
  -o, --output (path default cargo) change the default output dir for compilation
  -r, --run  don't compile, only re-run
  -S, --no-simplify by default, attempt to simplify rustc error messages
  -E, --edition (default '2018') Rust edition

  Cache Management:
  --add  (string...) add new crates to the cache
  --update update all, or a specific package given as argument
  --edit  edit the static cache Cargo.toml
  --build rebuild the static cache
  --cleanup clean out stale rlibs from cache
  --crates current crates and their versions in cache
  --doc  display documentation (any argument will be specific crate name)
  --edit-prelude edit the default prelude for snippets
  --alias (string...) crate aliases in form alias=crate_name (used with -x)

  Dynamic compilation:
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)
  -L, --link (string) path for extra libraries
  --cfg... (string) pass configuration variables to rustc
  --features (string...) enable features in compilation
  --libc  link dynamically against libc (special case)
  (--extern is used to explicitly link in a crate by name)

  -v, --verbose describe what's happening
  -V, --version version of runner

  <program> (string) Rust program, snippet or expression
  <args> (string...) arguments to pass to program
";

fn read_file_with_arg_comment(args: &mut lapp::Args, file: &Path) -> (String,bool) {
    let contents = fs::read_to_string(file).or_die("cannot read file");
    let first_line = contents.lines().next().or_die("empty file");
    let arg_comment = "//: ";
    let has_arg_comment = first_line.starts_with(arg_comment);
    if has_arg_comment {
        let default_args = &first_line[arg_comment.len()..];
        let default_args = shlex::split(default_args).or_die("bad comment args");
        args.parse_command_line(default_args).or_die("cannot parse comment args");
        args.clear_used();
    }
    (contents,has_arg_comment)

}

fn main() {
    let mut args = lapp::Args::new(USAGE);
    args.parse_spec().or_die("bad spec");
    let env = Path::new("env.rs");
    let env_prelude = if env.exists() {
        let (contents,_) = read_file_with_arg_comment(&mut args, env);
        Some(contents)
    } else {
        None
    };

    args.parse_env_args().or_die("bad command line");

    let program_contents = if let Ok(program) = args.get_string_result("program") {
        let prog = Path::new(&program);
        if program.ends_with(".rs") {
            if ! prog.is_file() {
                args.quit("file does not exist");
            }
            args.clear_used();
            let (contents,has_arg_comment) = read_file_with_arg_comment(&mut args, prog);
            if has_arg_comment {
                args.parse_env_args().or_die("bad command line");
            }
            Some(contents)
        } else {
            None
        }
    } else {
        None
    };

    let mut prelude = cache::get_prelude();
    if let Some(env_prelude) = env_prelude {
        prelude.insert_str(0, &env_prelude);
    }
    let b = |p| args.get_bool(p);

    let exe_suffix = if EXE_SUFFIX.len() > 0 {
        &EXE_SUFFIX[1..]
    } else {
        ""
    };

    if b("version") {
        println!("runner {}",VERSION);
        return;
    }
    let verbose = b("verbose");

    if b("run") && b("compile-only") {
        args.quit("--run and compile-only make no sense together");
    }

    let aliases = args.get_strings("alias");
    if aliases.len() > 0 {
        cache::add_aliases(aliases);
        return;
    }

    if b("edit-prelude") {
        let rdir = cache::runner_directory().join("prelude");
        edit(&rdir);
        return;
    }

    // Static Cache Management
    let crates = args.get_strings("add");
    if crates.len() > 0 {
        cache::create_static_cache(&crates);
        if program_contents.is_none() {
            return;
        }
    }

    // operations on the static cache
    let (edit_toml, build, doc, update, cleanup, crates) =
        (b("edit"), b("build"), b("doc"), b("update"), b("cleanup"), b("crates"));

    if edit_toml || build || doc || update || cleanup || crates {
        let maybe_argument = args.get_string_result("program");
        let static_cache = cache::static_cache_dir_check();
        if build || update {
            env::set_current_dir(&static_cache).or_die("static cache wasn't a directory?");
            if build {
                cache::build_static_cache();
            } else {
                if let Ok(package) = maybe_argument {
                    cache::cargo(&["update","--package",&package]);
                } else {
                    cache::cargo(&["update"]);
                }
                return;
            }
        } else
        if doc {
            let the_crate = crate_utils::proper_crate_name(
                &if let Ok(file) =  maybe_argument {
                    file
                } else {
                    "static_cache".to_string()
                }
            );
            let docs = static_cache.join(&format!("target/doc/{}/index.html",the_crate));
            open(&docs);
        } else
        if cleanup {
            cache::cargo(&["clean"]);
        } else
        if crates {
            let mut m = cache::get_metadata();
            let mut crates = Vec::new();
            if let Some(name) = maybe_argument.ok() {
                crates.push(name);
                crates.extend(args.get_strings("args"));
            }
            m.dump_crates(crates, verbose);
        } else { // must be edit_toml
            let toml = static_cache.join("Cargo.toml");
            edit(&toml);
        }
        return;
    }

    let first_arg = args.get_string("program");
    let file = PathBuf::from(&first_arg);
    let optimized = args.get_bool("optimize");
    let edition = args.get_string("edition");

    // Dynamically linking crates (experimental!)
    let (print_path, compile) = (b("crate-path"),b("compile"));
    if print_path || compile {
        let mut state = State::dll(optimized, &edition);
        // plain-jane name is a crate name!
        if crate_utils::plain_name(&first_arg) {
            // but is it one of Ours? Then we definitely know what the
            // actual crate name is AND where the source is cached
            let m = cache::get_metadata();
            if let Some(e) = m.get_meta_entry(&first_arg) {
                if e.path == Path::new("") {
                    args.quit("please run 'runner --build' to update metadata");
                }
                // will be <cargo dir>/src/FILE.rs
                let path = e.path.parent().unwrap().parent().unwrap();
                if print_path {
                    println!("{}",path.display());
                } else {
                    let ci = crate_utils::crate_info(&path.join("Cargo.toml"));
                    // respect the crate's edition!
                    state.edition = ci.edition;
                    // TBD can override --features with features actually
                    // used to build this crate
                    let build_features = &e.features;
                    println!("building crate '{}' {} at {}",e.crate_name, build_features, e.path.display());
                    compile_crate(&args, &state, &e.crate_name, &e.path, None,
                        Vec::new(),
                        build_features.split_whitespace().map(|s| s.to_string()).collect()
                    );
                }
                return;
            }
        } else
        if compile { // either a cargo directory or a Rust source file
            if ! file.exists() {
                args.quit("no such file or directory");
            }
            let (crate_name, crate_path) = if file.is_dir() {
                match crate_utils::cargo_dir(&file) {
                    Ok((path,cargo_toml)) => {
                        // this is somewhat dodgy, since the default location can be changed
                        // Safest bet is to add the crate to the runner static cache
                        let source = path.join("src").join("lib.rs");
                        let ci = crate_utils::crate_info(&cargo_toml);
                        // respect the crate's edition!
                        state.edition = ci.edition;
                        (ci.name, source)
                    },
                    Err(msg) => args.quit(&msg)
                }
            } else { // should be just a Rust source file
                if file.extension().or_die("expecting extension") != "rs" {
                    args.quit("expecting known crate, dir containing Cargo.toml or Rust source file");
                }
                let name = crate_utils::path_file_name(&file.with_extension(""));
                (name, file.clone())
            };
            println!("building crate '{}' at {}",crate_name, crate_path.display());
            compile_crate(&args, &state, &crate_name, &crate_path, None,  Vec::new(),Vec::new());
            return;
        } else { // we no longer go for wild goose chase to find crates in the Cargo cache
            args.quit("not found in the static cache");
        }
    }

    let static_state = b("static") && ! b("dynamic");
    let state = State::exe(static_state,optimized, &edition);

    // we'll pass rest of arguments to program
    let program_args = args.get_strings("args");

    let mut expression = true;
    use cache::quote;
    let mut code = if b("expression") {
        // Evaluating an expression: just debug print it out.
        format!("println!(\"{{:?}}\",{});", quote(first_arg))
    } else
    if b("iterator") {
        // The expression is anything that implements IntoIterator
        format!("for val in {} {{\n println!(\"{{:?}}\",val);\n}}", quote(first_arg))
    } else
    if b("lines") {
        // The variable 'line' is available to an expression, evaluated for each line in stdin
        // But if the expression ends with '}' then don't dump out this value!
        let first_arg = quote(first_arg);
        let stmt = first_arg.trim_end().ends_with('}');
        let mut s = String::from("
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line?;
        ");
        s += &if ! stmt {
            format!("let val = {};\nprintln!(\"{{:?}}\",val);", first_arg)
        } else {
            format!("  {};",first_arg)
        };
        s += "\n}";
        s
    } else { // otherwise, just a file
        expression = false;
        program_contents.or_die("no .rs file")
    };

    // ALL executables go into the Runner bin directory...
    let mut bin = cache::runner_directory().join("bin");
    let mut externs = Vec::new();

    // proper Rust programs are accepted (this is a bit rough)
    let proper = code.find("fn main").is_some();
    let (rust_file, program) = if ! proper {
        // otherwise we must create a proper program from the snippet
        // and write this as a file in the Runner bin directory...
        let mut extern_crates = args.get_strings("extern");
        let wild_crates = args.get_strings("wild");
        let macro_crates = args.get_strings("macro");
        if wild_crates.len() > 0 {
            extern_crates.extend(wild_crates.iter().cloned());
        }
        if macro_crates.len() > 0 {
            extern_crates.extend(macro_crates.iter().cloned());
        }
        let macro_crates: HashSet<_> = macro_crates.into_iter().collect();

        let mut extra = args.get_string("prepend");
        if ! extra.is_empty() {
            extra.push(';');
        }
        let maybe_prelude = if b("no-prelude") {
            "".into()
        } else {
            prelude
        };

        let (massaged_code, deduced_externs)
            = massage_snippet(code, maybe_prelude, extern_crates, wild_crates, macro_crates, extra, edition=="2018");
        code = massaged_code;
        externs = deduced_externs;
        if ! expression {
            bin.push(file.file_name().unwrap());
            bin.set_extension("rs");
        } else { // we make up a name...
            bin.push("tmp.rs");
        }
        fs::write(&bin,&code).or_die("cannot write code");
        let program = bin.with_extension(exe_suffix);
        (bin, program)
    } else {
        for line in code.lines() {
            if let Some(crate_name) = strutil::word_after(line,"extern crate ") {
                externs.push(crate_name);
            }
        }
        // the 'proper' case - use the file name part
        bin.push(file.file_name().unwrap());
        let program = bin.with_extension(exe_suffix);
        (file, program)
    };

    if b("run") {
        if ! program.exists() {
            args.quit(&format!("program {:?} does not exist",program));
        }
    } else {
        if ! compile_crate(&args,&state,"",&rust_file,Some(&program), externs, Vec::new()) {
            process::exit(1);
        }
        if verbose {
            println!("compiled {:?} successfully",rust_file);
        }
    }

    if b("compile-only") {
        let file_name = rust_file.file_name().or_die("no file name?");
        let out_dir = args.get_path("output");
        let home = if out_dir == Path::new("cargo") {
            let home = crate_utils::cargo_home().join("bin");
            if ! home.is_dir() {
                // With Windows, standalone installer does not create this directory
                // (may well be a Bugge)
                fs::create_dir(&home).or_die("could not create Cargo bin directory");
                println!("creating Cargo bin directory {}\nEnsure it is on your PATH",home.display());
            }
            home
        } else {
            out_dir
        };
        let here = home.join(file_name).with_extension(exe_suffix);
        println!("Copying {} to {}",program.display(),here.display());
        fs::copy(&program,&here).or_die("cannot copy program");
        return;
    }

    // Finally run the compiled program
    let ch = cache::get_cache(&state);
    let mut builder = process::Command::new(&program);
    if ! state.build_static {
        // must make the dynamic cache visible to the program!
        if cfg!(windows) {
            // Windows resolves DLL references on the PATH
            let path = env::var("PATH").unwrap();
            let new_path = format!("{};{}",path,ch.display());
            builder.env("PATH",new_path);
        } else {
            // whereas POSIX requires LD_LIBRARY_PATH
            builder.env("LD_LIBRARY_PATH",format!("{}:{}",*RUSTUP_LIB,ch.display()));
        }
        builder.env(
            "DYLD_FALLBACK_LIBRARY_PATH",
            format!("{}:{}", ch.display(), *RUSTUP_LIB),
        );

    }
    let status = builder.args(&program_args)
        .status()
        .or_then_die(|e| format!("can't run program {:?}: {}",program,e));

    if ! status.success() {
        process::exit(status.code().unwrap_or(-1));
    }
}
