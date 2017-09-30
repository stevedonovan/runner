//! A Rust snippet runner.
//!
//! Please see [readme](https://github.com/stevedonovan/runner/blob/master/readme.md)
extern crate easy_shortcuts as es;
extern crate lapp;
use es::traits::*;
use std::process;
use std::env;
use std::fs;
use std::path::{Path,PathBuf};
use std::collections::HashMap;
use std::io::Write;

mod crate_utils;
mod platform;
mod strutil;

use std::env::consts::{EXE_SUFFIX,DLL_SUFFIX,DLL_PREFIX};

use platform::{open,edit};

const USAGE: &str = "
Compile and run small Rust snippets
  -s, --static build statically (default is dynamic)
  -O, --optimize optimized static build
  -e, --expression evaluate an expression
  -i, --iterator evaluate an iterator
  -n, --lines evaluate expression over stdin; 'line' is defined
  -x, --extern... (string) add an extern crate to the snippet
  -X, --wild... (string) like -x but implies wildcard use
  -p, --prepend (default '') prepend contents of this file to body

  Cache Management:
  --create (string...) initialize the static cache with crates
  --add  (string...) add new crates to the cache (after --create)
  --edit  edit the static cache Cargo.toml
  --build rebuild the static cache
  --doc  display documentation (any argument will be specific crate name)
  --edit-prelude edit the default prelude for snippets
  --alias (string...) crate aliases in form alias=crate_name (used with -x)

  Dynamic compilation:
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)
  --cfg... (string) pass configuration variables to rustc
  --features (string...) enable features in compilation
  --libc  link dynamically against libc (special case)
  (--extern is used to explicitly link in a crate by name)

  <program> (string) Rust program, snippet or expression
  <args> (string...) arguments to pass to program
";

// this will be initially written to ~/.cargo/.runner/prelude and
// can then be edited.
const PRELUDE: &'static str = "
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_macros)]
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::env;
use std::path::{PathBuf,Path};

macro_rules! debug {
    ($x:expr) => {
        println!(\"{} = {:?}\",stringify!($x),$x);
    }
}
";

#[derive(Default)]
struct State {
    build_static: bool,
    optimize: bool,
    exe: bool,
}

fn main() {
    let args = lapp::parse_args(USAGE);
    let prelude = get_prelude();

    let aliases = args.get_strings("alias");
    if aliases.len() > 0 {
        add_aliases(aliases);
        return;
    }

    if args.get_bool("edit-prelude") {
        let rdir = runner_directory().join("prelude");
        edit(&rdir);
        return;
    }

    // Static Cache Management
    let crates = args.get_strings("create");
    if crates.len() > 0 {
        create_static_cache(&crates,true);
        return;
    }
    let crates = args.get_strings("add");
    if crates.len() > 0 {
        static_cache_dir_check(&args);
        create_static_cache(&crates,false);
        return;
    }

    if args.get_bool("edit") || args.get_bool("build") || args.get_bool("doc") {
        let static_cache = static_cache_dir_check(&args);
        if args.get_bool("build") {
            env::set_current_dir(&static_cache).or_die("static cache wasn't a directory?");
            build_static_cache();
        } else
        if args.get_bool("doc") {
            let the_crate = crate_utils::proper_crate_name(
                &if let Ok(file) = args.get_string_result("program") {
                    file
                } else {
                    "static_cache".to_string()
                }
            );
            let docs = static_cache.join(&format!("target/doc/{}/index.html",the_crate));
            open(&docs);
        } else {
            let toml = static_cache.join("Cargo.toml");
            edit(&toml);
        }
        return;
    }

    let first_arg = args.get_string("program");
    let file = PathBuf::from(&first_arg);

    // Dynamically linking crates (experimental!)
    if args.get_bool("crate-path") || args.get_bool("compile") {
        let (crate_path,crate_name) = match crate_utils::crate_path(&file,&first_arg) {
            Ok(t) => t,
            Err(s) => args.quit(&s)
        };
        if args.get_bool("crate-path") {
            println!("{}",crate_utils::cache_path(&crate_name).display());
        } else {
            println!("building crate '{}' at {}",crate_name, crate_path.display());
            compile_crate(&args, &Default::default(), &crate_name, &crate_path, None);
        }
        return;
    }

    let state = State {
        build_static: args.get_bool("static"),
        optimize: args.get_bool("optimize"),
        exe: true,
    };

    // we'll pass rest of arguments to program
    let program_args = args.get_strings("args");

    let mut expression = true;
    let mut code = if args.get_bool("expression") {
        // Evaluating an expression: just print it out.
        format!("println!(\"{{:?}}\",{});", first_arg)
    } else
    if args.get_bool("iterator") {
        // The expression is anything that implements IntoIterator
        format!("let iter = {};\n for val in iter {{ println!(\"{{:?}}\",val);}}", first_arg)
    } else
    if args.get_bool("lines") {
        // The variable 'line' is available to an expression, evaluated for each line in stdin
        format!("
            let stdin = io::stdin();
            for line in stdin.lock().lines() {{
                let line = line?;
                let val = {};
                println!(\"{{:?}}\",val);
            }}
            ", first_arg)
    } else { // otherwise, just a file
        expression = false;
        es::read_to_string(&file)
    };

    // expressions may contain environment references like $PATH
    if expression {
        code = strutil::substitute(&code,"$",
            |c| c.is_alphanumeric() || c == '_',
            |s| {
                let text = if let Ok(num) = s.parse::<usize>() {
                    program_args.get(num-1).or_die(&format!("arg {} not found",num)).clone()
                } else {
                    env::var(s).or_die("$VAR not found")
                };
                format!("{:?}",text)
            }
        );
    }

    // ALL executables go into the Runner bin directory...
    let mut bin = runner_directory().join("bin");

    // proper Rust programs are accepted (this is a bit rough)
    let proper = code.find("fn main").is_some();
    let (rust_file, program) = if ! proper {
        // otherwise we must create a proper program from the snippet
        // and write this as a file in the Runner bin directory...
        let mut extern_crates = args.get_strings("extern");
        let wild_crates = args.get_strings("wild");
        if wild_crates.len() > 0 {
            extern_crates.extend(wild_crates.iter().cloned());
        }
        let mut extra = args.get_string("prepend");
        if extra != "" {
            extra = es::read_to_string(&extra);
        }
        code = massage_snippet(code, prelude, extern_crates, wild_crates, extra);
        if ! expression {
            bin.push(&file);
            bin.set_extension("rs");
        } else {
            bin.push("tmp.rs");
        }
        es::write_all(&bin,&code);
        let program = bin.with_extension(EXE_SUFFIX);
        (bin, program)
    } else {
        bin.push(&file);
        let program = bin.with_extension(EXE_SUFFIX);
        (file, program)
    };

    if ! compile_crate(&args,&state,"",&rust_file,Some(&program)) {
        return;
    }


    // Finally run the compiled program
    let cache = get_cache(&state);
    let mut builder = process::Command::new(&program);
    if ! state.build_static {
        builder.env("LD_LIBRARY_PATH",format!("{}:{}",crate_utils::rustup_lib(),cache.display()));
    }
    builder.args(&program_args)
        .status()
        .or_die(&format!("can't run program {:?}",program));
}

// handle two useful cases:
// - compile a crate as a dynamic library, given a name and an output dir
// - compile a program, given a program
fn compile_crate(args: &lapp::Args, state: &State,
    crate_name: &str, crate_path: &Path,
    output_program: Option<&Path>) -> bool
{
    // implicit linking works fine, until it doesn't
    let mut extern_crates = args.get_strings("extern");
    // libc is such a special case
    if args.get_bool("libc") {
        extern_crates.push("libc".into());
    }
    let mut cfg = args.get_strings("cfg");
    for f in args.get_strings("features") {
        cfg.push(format!("feature=\"{}\"",f));
    }
    let cache = get_cache(&state);
    let mut builder = process::Command::new("rustc");
    if ! state.build_static { // stripped-down dynamic link
        builder.args(&["-C","prefer-dynamic"]).args(&["-C","debuginfo=0"]);
    } else { // static build
        builder.arg(if state.optimize {"-O"} else {"-g"});
    }
    // implicitly linking against crates in the dynamic or static cache
    builder.arg("-L").arg(&cache);
    if ! state.exe { // as a dynamic library
        builder.args(&["--crate-type","dylib"])
        .arg("--out-dir").arg(&cache)
        .arg("--crate-name").arg(&crate_utils::proper_crate_name(crate_name));
    } else {
        builder.arg("-o").arg(output_program.unwrap());
    }
    for c in cfg {
        builder.arg("--cfg").arg(&c);
    }
    for c in extern_crates {
        let ext = format!("{}={}/{}{}{}",c,cache.display(),DLL_PREFIX,c,DLL_SUFFIX);
        builder.arg("--extern").arg(&ext);
    }
    builder.arg(crate_path);
    builder.status().or_die("can't run rustc").success()
}


fn runner_directory() -> PathBuf {
    crate_utils::cargo_home().join(".runner")
}

fn cargo(args: &[&str]) -> bool {
    let res = process::Command::new("cargo")
        .args(args)
        .status()
        .or_die("can't run cargo");
    res.success()
}

const STATIC_CACHE: &str = "static-cache";
const DYNAMIC_CACHE: &str = "dy-cache";

fn static_cache_dir() -> PathBuf {
    runner_directory().join(STATIC_CACHE)
}

fn static_cache_dir_check(args: &lapp::Args) -> PathBuf {
    let static_cache = static_cache_dir();
    if ! static_cache.exists() {
        args.quit("please build static cache with --create first");
    }
    static_cache
}

fn build_static_cache() -> bool {
    cargo(&["build"]) &&
    cargo(&["build","--release"]) &&
    cargo(&["doc"])
}

fn create_static_cache(crates: &[String], create: bool) {
    use std::io::prelude::*;
    let mut home = runner_directory();
    env::set_current_dir(&home).or_die("cannot change to home directory");
    if create {
        if ! cargo(&["new","--bin",STATIC_CACHE]) {
            es::quit("cannot create static cache");
        }
    }
    home.push(STATIC_CACHE);
    env::set_current_dir(&home).or_die("could not change to static cache directory");
    let tmpfile = env::temp_dir().join("Cargo.toml");
    fs::copy("Cargo.toml",&tmpfile).or_die("cannot back up Cargo.toml");
    {
        let mut deps = fs::OpenOptions::new().append(true)
            .open("Cargo.toml").or_die("could not append to Cargo.toml");
        for c in crates {
            write!(deps,"{}=\"*\"\n",c).or_die("could not modify Cargo.toml");
        }
    }
    if ! build_static_cache() {
        println!("Error occurred - restoring Cargo.toml");
        fs::copy(&tmpfile,"Cargo.toml").or_die("cannot restore Cargo.toml");
    }
}

// this is always called first and has the important role to ensure that
// runner's directory structure is created properly.
fn get_prelude() -> String {
    let home = runner_directory();
    let pristine = ! home.is_dir();
    if pristine {
        fs::create_dir(&home).or_die("cannot create runner directory");
    }
    let prelude = home.join("prelude");
    let bin = home.join("bin");
    if pristine {
        es::write_all(&prelude,PRELUDE);
        fs::create_dir(&home.join(DYNAMIC_CACHE)).or_die("cannot create dynamic cache");
    }
    if pristine || ! bin.is_dir() {
        fs::create_dir(&bin).or_die("cannot create output directory");
    }
    es::read_to_string(&prelude)
}

fn get_cache(state: &State) -> PathBuf {
    let mut home = runner_directory();
    let scache;
    let cache = if state.build_static {
        let dir = if state.optimize {"release"} else {"debug"};
        scache = format!("{}/target/{}/deps",STATIC_CACHE,dir);
        &scache
    } else {DYNAMIC_CACHE};
    home.push(cache);
    home
}

fn add_aliases(aliases: Vec<String>) {
    if aliases.len() == 0 { return; }
    let alias_file = runner_directory().join("alias");
    let mut f = if alias_file.is_file() {
        fs::OpenOptions::new().append(true).open(&alias_file)
    } else {
        fs::File::create(&alias_file)
    }.or_die("cannot open runner alias file");

    for crate_alias in aliases {
        write!(f,"{}\n",crate_alias).or_die("cannot write to runner alias file");
    }
}

fn get_aliases() -> HashMap<String,String> {
    let alias_file = runner_directory().join("alias");
    if ! alias_file.is_file() { return HashMap::new(); }
	es::lines(es::open(&alias_file))
      .filter_map(|s| s.split_at_delim('=').trim()) // split into (String,String)
	  .to_map()
}


fn massage_snippet(code: String, prelude: String, extern_crates: Vec<String>, wild_crates: Vec<String>, body_prelude: String) -> String {
    fn indent_line(line: &str) -> String {
        format!("    {}\n",line)
    }
    let mut prefix = prelude;
    let mut body = String::new();
    body += &body_prelude;
    {
        if extern_crates.len() > 0 {
            let aliases = get_aliases();
            for c in extern_crates {
                prefix += &if let Some(aliased) = aliases.get(&c) {
                    format!("extern crate {} as {};",aliased,c)
                } else {
                    format!("extern crate {};",c)
                };
            }
            for c in wild_crates {
                prefix += &format!("use {}::*;",c);
            }
        }
        let mut lines = code.lines();
        let mut first = true;
        for line in lines.by_ref() {
            let line = line.trim_left();
            if first { // files may start with #! shebang...
                if line.starts_with("#!") {
                    continue;
                }
                first = false;
            }
            // crate import, use should go at the top.
            // Particularly need to force crate-level attributes to the top
            // - currently only 'macro_use'.
            if line.starts_with("//") || line.starts_with("#[macro_use") ||
                line.starts_with("extern ") || line.starts_with("use ") {
                prefix += line;
                prefix.push('\n');
            } else
            if line.len() > 0 {
                body += &indent_line(line);
                break;
            }
        }
        // and indent the rest!
        body.extend(lines.map(indent_line));
    }

    format!("{}
use std::error::Error;
fn run() -> Result<(),Box<Error>> {{
{}    Ok(())
}}
fn main() {{
    if let Err(e) = run() {{
        println!(\"error: {{:?}}\",e);
    }}
}}
",prefix,body)

}

