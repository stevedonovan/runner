//! A Rust snippet runner.
//!
//! Please see [readme](https://github.com/stevedonovan/runner/blob/master/readme.md)
extern crate easy_shortcuts as es;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

use es::traits::Die;
use lapp::Args;
use std::collections::HashSet;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use std::string::ToString;
use std::{env, io};

mod cache;
mod cargo_lock;
mod compile;
mod crate_utils;
mod meta;
mod platform;
mod state;
mod strutil;

use cache::quote;
use compile::{check_well_formed, compile_crate, massage_snippet};
use crate_utils::RUSTUP_LIB;
use platform::{edit, open};
use state::State;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "
Compile and run small Rust snippets
  -s, --static build statically (default is dynamic)
  -d, --dynamic overrides --static in env.rs
  -O, --optimize optimized static build
  -e, --expression evaluate an expression - try enclosing in braces if having trouble
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
  -E, --edition (default '2021') specify Rust edition
  -I, --stdin Input from stdin

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

/// Read source file and interpret any arguments prefixed by "//: ".
fn read_file_with_arg_comment(args: &mut Args, file: &Path) -> (String, bool) {
    let contents = fs::read_to_string(file).or_die("cannot read file");
    let first_line = contents.lines().next().or_die("empty file");
    let arg_comment = "//: ";
    let has_arg_comment = first_line.starts_with(arg_comment);
    if has_arg_comment {
        let default_args = &first_line[arg_comment.len()..];
        if args.get_bool("verbose") {
            eprintln!(
                "Picked up arguments from {:?}: {default_args}",
                file.file_name()
                    .or_then_die(|e| format!("error retrieving filename: [{e}]"))
            );
        }

        let default_args = shlex::split(default_args).or_die("bad comment args");
        args.parse_command_line(default_args)
            .or_die("cannot parse comment args");
        args.clear_used();
    }
    (contents, has_arg_comment)
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn main() {
    let start = std::time::Instant::now();
    let mut args = Args::new(USAGE);
    args.parse_spec().or_die("bad spec");
    args.parse_env_args().or_die("bad command line");

    // let b = |p| args.get_bool(p);

    let program_contents = if let Ok(program) = args.get_string_result("program") {
        let prog = Path::new(&program);
        if program.ends_with(".rs") {
            if args.get_bool("compile-only") && args.get_bool("stdin") {
                None
            } else if prog.is_file() {
                args.clear_used();
                let (contents, has_arg_comment) = read_file_with_arg_comment(&mut args, prog);
                if has_arg_comment {
                    args.parse_env_args().or_die("bad command line");
                }
                Some(contents)
            } else {
                args.quit("file does not exist");
            }
        } else {
            None
        }
    } else {
        None
    };
    // eprintln!("program_contents={program_contents:?}");
    let env = Path::new("env.rs");
    eprintln!("env path={env:?}, env exists={}", env.exists());
    let env_prelude = if env.exists() {
        let (contents, _) = read_file_with_arg_comment(&mut args, env);
        eprintln!("contents={contents}");
        Some(contents)
    } else {
        None
    };

    let mut prelude = cache::get_prelude();
    if let Some(env_prelude) = env_prelude {
        prelude.insert_str(0, &env_prelude);
    }
    let b = |p| args.get_bool(p);

    let exe_suffix = if EXE_SUFFIX.is_empty() {
        ""
    } else {
        &EXE_SUFFIX[1..]
    };

    if b("version") {
        println!("runner {VERSION}");
        return;
    }
    let verbose = b("verbose");

    if b("run") && b("compile-only") {
        args.quit("--run and compile-only make no sense together");
    }
    if b("lines") && b("stdin") {
        args.quit("--lines and stdin make no sense together, as lines already reads from stdin");
    }

    let aliases = args.get_strings("alias");
    if !aliases.is_empty() {
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
    if !crates.is_empty() {
        cache::create_static(&crates);
        if program_contents.is_none() {
            return;
        }
    }

    // operations on the static cache
    let (edit_toml, build, doc, update, cleanup, crates) = (
        b("edit"),
        b("build"),
        b("doc"),
        b("update"),
        b("cleanup"),
        b("crates"),
    );

    if edit_toml || build || doc || update || cleanup || crates {
        let maybe_argument = args.get_string_result("program");
        let static_cache = cache::static_cache_dir_check();
        if build || update {
            env::set_current_dir(&static_cache).or_die("static cache wasn't a directory?");
            if build {
                cache::build_static();
            } else {
                if let Ok(package) = maybe_argument {
                    cache::cargo(&["update", "--package", &package]);
                } else {
                    cache::cargo(&["update"]);
                }
                return;
            }
        } else if doc {
            let the_crate = crate_utils::proper_crate_name(&if let Ok(file) = maybe_argument {
                file
            } else {
                "static_cache".to_string()
            });
            let docs = static_cache.join(format!("target/doc/{the_crate}/index.html"));
            open(&docs);
        } else if cleanup {
            cache::cargo(&["clean"]);
        } else if crates {
            let mut m = cache::get_metadata();
            let mut crates = Vec::new();
            if let Ok(name) = maybe_argument {
                crates.push(name);
                crates.extend(args.get_strings("args"));
            }
            m.dump_crates(crates, verbose);
        } else {
            // must be edit_toml
            let toml = static_cache.join("Cargo.toml");
            edit(&toml);
        }
        return;
    }

    // Run Rust code
    let static_state = b("static") && !b("dynamic");

    if b("run") {
        let mode_req = if b("static") { "static" } else { "dynamic" };
        eprintln!("Flag --{mode_req} will be ignored since program is precompiled");
    }

    // eprintln!(
    //     "b(\"stdin\")={}; b(\"compile-only\")={}",
    //     b("stdin"),
    //     b("compile-only")
    // );
    let (first_arg_opt, file_res): (Option<String>, Option<PathBuf>) =
        if b("stdin") && !b("compile-only") {
            // "STDIN".to_string()
            (None, None)
        } else {
            // eprintln!("About to call args.get_string(\"program\")...");
            let first_arg = args.get_string("program");
            // eprintln!("... it returned first_arg={first_arg}");
            (Some(first_arg.clone()), Some(PathBuf::from(first_arg)))
        };
    // let file = PathBuf::from(&first_arg);
    let optimized = args.get_bool("optimize");
    let edition = args.get_string("edition");

    // Dynamically linking crates (experimental!)
    let (print_path, compile) = (b("crate-path"), b("compile"));
    if print_path || compile {
        let mut state = State::dll(optimized, &edition);
        // plain-jane name is a crate name!
        let Some(first_arg) = &first_arg_opt else {
            args.quit("build requested with no filename")
        };
        if crate_utils::plain_name(first_arg) {
            // but is it one of Ours? Then we definitely know what the
            // actual crate name is AND where the source is cached
            let m = cache::get_metadata();
            if let Some(e) = m.get_meta_entry(first_arg) {
                if e.path == Path::new("") {
                    args.quit("please run 'runner --build' to update metadata");
                }
                // will be <cargo dir>/src/FILE.rs
                let path = e.path.parent().unwrap().parent().unwrap();
                if print_path {
                    println!("{}", path.display());
                } else {
                    let ci = crate_utils::crate_info(&path.join("Cargo.toml"));
                    // respect the crate's edition!
                    state.edition = ci.edition;
                    // TBD can override --features with features actually
                    // used to build this crate
                    let build_features = &e.features;
                    eprintln!(
                        "dynamically linking crate '{}' with features [{}] at {}",
                        e.crate_name,
                        build_features,
                        e.path.display()
                    );
                    compile_crate(
                        &args,
                        &state,
                        &e.crate_name,
                        &e.path,
                        None,
                        Vec::new(),
                        build_features
                            .split_whitespace()
                            .map(ToString::to_string)
                            .collect(),
                    );
                }
                return;
            }
        } else {
            if compile {
                if let Some(file) = file_res {
                    if !file.exists() {
                        args.quit("no such file or directory for crate compile");
                    }
                    let (crate_name, crate_path) = if file.is_dir() {
                        match crate_utils::cargo_dir(&file) {
                            Ok((path, cargo_toml)) => {
                                // this is somewhat dodgy, since the default location can be changed
                                // Safest bet is to add the crate to the runner static cache
                                let source = path.join("src").join("lib.rs");
                                let ci = crate_utils::crate_info(&cargo_toml);
                                // respect the crate's edition!
                                state.edition = ci.edition;
                                (ci.name, source)
                            }
                            Err(msg) => args.quit(&msg),
                        }
                    } else {
                        // should be just a Rust source file
                        if file.extension().or_die("expecting extension") != "rs" {
                            args.quit(
                                "expecting known crate, dir containing Cargo.toml or Rust source file",
                            );
                        }
                        let name = crate_utils::path_file_name(&file.with_extension(""));
                        (name, file.clone())
                    };
                    eprintln!(
                        "compiling crate '{}' at {}",
                        crate_name,
                        crate_path.display()
                    );
                    compile_crate(
                        &args,
                        &state,
                        &crate_name,
                        &crate_path,
                        None,
                        Vec::new(),
                        Vec::new(),
                    );
                    return;
                }
                args.quit("--compile specified with no crate name");
            }
            // we no longer go for wild goose chase to find crates in the Cargo cache
            args.quit("not found in the static cache");
        }
    }

    let state = State::exe(static_state, optimized, &edition);

    // we'll pass rest of arguments to program
    let program_args = args.get_strings("args");

    let mut expression = true;
    let (has_save_name, raw_code) = if b("stdin") {
        let mut s = String::new();

        // Read lines from stdin in a loop until EOF is reached
        loop {
            let bytes_read = io::stdin()
                .read_line(&mut s)
                .or_die("could not read from stdin");
            if bytes_read == 0 {
                break; // EOF reached
            }
        }

        // println!("Content from stdin:\n{}", s);
        (b("compile-only"), quote(s))
    } else if b("expression") || b("iterator") || b("lines") {
        // let file = file_res.clone().or_die("no such file or directory");

        let first_arg = first_arg_opt.or_die("No Rust source file specified");

        (false, quote(first_arg.clone()))
    } else {
        // otherwise, just a file
        expression = false;
        (true, program_contents.or_die("no .rs file"))
    };

    let well_formed = if b("iterator") || b("lines") {
        false
    } else {
        // eprintln!("Checking if snippet has an fn main, and if so, does it compile?...");
        check_well_formed(verbose, &raw_code)
    };

    let mut code = if b("expression") {
        if well_formed {
            raw_code
        } else {
            // Evaluating an expression: just debug print it out.
            let expr_code = format!("println!(\"{{:?}}\",{});", raw_code.trim_end());
            // eprintln!("\nexpr_code={expr_code}\n");
            expr_code
        }
    } else if b("iterator") {
        // The expression is anything that implements IntoIterator
        format!("for val in {raw_code} {{\n println!(\"{{:?}}\",val);\n}}")
    } else if b("lines") {
        // The variable 'line' is available to an expression, evaluated for each line in stdin
        // But if the expression ends with '}' then don't dump out this value!
        let mut s = String::from(
            "
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line?;
        ",
        );
        s += &if raw_code.trim_end().ends_with('}') {
            format!("  {raw_code};")
        } else {
            format!("let val = {raw_code};\nprintln!(\"{{:?}}\",val);")
        };
        s += "\n}";
        s
    } else {
        raw_code.trim_end().to_string()
    };

    // ALL executables go into the Runner bin directory...
    let mut bin = cache::runner_directory().join("bin");
    let mut externs = Vec::new();

    // Well-formed Rust programs are accepted
    let (rust_file, program) = if well_formed {
        for line in code.lines() {
            if let Some(crate_name) = strutil::word_after(line, "extern crate ") {
                externs.push(crate_name);
            }
        }
        // the 'proper' case - use the file name part
        let file = file_res.or_die("no such file or directory for code containing 'fn main'");
        bin.push(file.file_name().unwrap());
        let program = bin.with_extension(exe_suffix);
        (file, program)
    } else {
        // otherwise we must create a proper program from the snippet
        // and write this as a file in the Runner bin directory...
        let mut extern_crates = args.get_strings("extern");
        let wild_crates = args.get_strings("wild");
        let macro_crates = args.get_strings("macro");
        if !wild_crates.is_empty() {
            extern_crates.extend(wild_crates.iter().cloned());
        }
        if !macro_crates.is_empty() {
            extern_crates.extend(macro_crates.iter().cloned());
        }
        let macro_crates: HashSet<_> = macro_crates.into_iter().collect();

        let mut extra = args.get_string("prepend");
        if !extra.is_empty() {
            // eprintln!(
            //     "1. before: extra={extra:?}, extra.as_bytes()={:?}",
            //     extra.as_bytes()
            // );
            extra = extra.replace("\\n", "\n"); // Issue #5: undo escaping to restore what the user entered
            extra.push(';');
            extra.push('\n'); // Issue #5 Add a line feed to separate extra section from body
                              // eprintln!(
                              //     "2. after: extra={extra}, extra.as_bytes()={:?}",
                              //     extra.as_bytes()
                              // );
        }
        let maybe_prelude = if b("no-prelude") {
            String::new()
        } else {
            prelude
        };

        let (massaged_code, deduced_externs) = massage_snippet(
            &code,
            maybe_prelude,
            extern_crates,
            wild_crates,
            &macro_crates,
            &extra,
            edition == "2021",
            verbose,
        );
        code = massaged_code;
        externs = deduced_externs;
        if expression && !has_save_name {
            // we make up a name...
            bin.push("tmp.rs");
        } else {
            let file = file_res.clone().or_die("no such file or directory");
            bin.push(file.file_name().unwrap());
            bin.set_extension("rs");
        }
        fs::write(&bin, &code).or_die("cannot write code");
        let program = bin.with_extension(exe_suffix);
        (bin, program)
    };

    // Compile program unless running precompiled
    if b("run") {
        if !program.exists() {
            args.quit(&format!("program {program:?} does not exist"));
        }
    } else {
        if verbose {
            eprintln!("Building program ({program:?}) from source {rust_file:?}",);
            let mode_stem = if state.build_static { "stat" } else { "dynam" };
            eprintln!("Compiling {mode_stem}ically");
        };
        if !compile_crate(
            &args,
            &state,
            "",
            &rust_file,
            Some(&program),
            externs,
            Vec::new(),
        ) {
            process::exit(1);
        }
        if verbose {
            println!("Compiled {rust_file:?} successfully to {program:?}");
        }
    }

    if b("compile-only") {
        // copy and return
        let file_name = rust_file.file_name().or_die("no file name?");
        let out_dir = args.get_path("output");
        let home = if out_dir == Path::new("cargo") {
            let home = crate_utils::cargo_home().join("bin");
            if !home.is_dir() {
                // With Windows, standalone installer does not create this directory
                // (may well be a Bugge)
                fs::create_dir(&home).or_die("could not create Cargo bin directory");
                println!(
                    "creating Cargo bin directory {}\nEnsure it is on your PATH",
                    home.display()
                );
            }
            home
        } else {
            out_dir
        };
        let here = home.join(file_name).with_extension(exe_suffix);
        println!("Copying {} to {}", program.display(), here.display());
        fs::copy(&program, &here).or_die("cannot copy program");
        return;
    }

    // Finally run the compiled program
    let ch = cache::get_cache(&state);
    let mut builder = process::Command::new(&program);
    if state.build_static {
        if verbose && !b("run") {
            eprintln!("Running statically");
        }
    } else {
        if verbose && !b("run") {
            eprintln!("Running program ({program:?}) dynamically");
        }
        // must make the dynamic cache visible to the program!
        if cfg!(windows) {
            // Windows resolves DLL references on the PATH
            let path = env::var("PATH").unwrap();
            let new_path = format!("{};{}", path, ch.display());
            builder.env("PATH", new_path);
        } else {
            // whereas POSIX requires LD_LIBRARY_PATH
            builder.env(
                "LD_LIBRARY_PATH",
                format!("{}:{}", *RUSTUP_LIB, ch.display()),
            );
        }
        builder.env(
            "DYLD_FALLBACK_LIBRARY_PATH",
            format!("{}:{}", ch.display(), *RUSTUP_LIB),
        );
    }

    if verbose {
        eprintln!(
            "Running {program:?} with environment [{:?}] and args [{:?}]",
            builder.get_envs(),
            builder.get_args()
        );
    }

    if verbose {
        eprintln!("About to execute program {builder:?}");
    }

    let dash_line = "-".repeat(50);
    println!("{dash_line}");
    let status = builder
        .args(&program_args)
        .status()
        .or_then_die(|e| format!("can't run program {program:?}: {e}"));

    if !status.success() {
        process::exit(status.code().unwrap_or(-1));
    }

    println!("{dash_line}");
    if verbose {
        let dur = start.elapsed();
        eprintln!("Completed in {}.{}s", dur.as_secs(), dur.subsec_millis());
    }
}
