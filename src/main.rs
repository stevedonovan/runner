//! A Rust snippet runner.
//!
//! Please see [readme](https://github.com/stevedonovan/runner/blob/master/readme.md)
extern crate easy_shortcuts as es;
extern crate lapp;
use es::traits::*;
use std::process;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use std::io::Write;

mod crate_utils;
mod platform;

use platform::{open,edit,EXE,SO};

fn rustup_lib() -> String {
    es::shell("rustc --print sysroot") + "/lib"
}

// this will be initially written to ~/.cargo/.runner/prelude and
// can then be edited.
const PRELUDE: &'static str = "
#![allow(unused_imports)]
#![allow(dead_code)]
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::env;
use std::path::{PathBuf,Path};
#[allow(unused_macros)]
macro_rules! debug {
    ($x:expr) => {
        println!(\"{} = {:?}\",stringify!($x),$x);
    }
}

";

fn runner_directory() -> PathBuf {
    let mut home = crate_utils::cargo_home();
    home.push(".runner");
    home
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

fn get_cache(build_static: bool, optimize: bool) -> PathBuf {
    let mut home = runner_directory();
    let scache;
    let cache = if build_static {
        let dir = if optimize {"release"} else {"debug"};
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

fn massage_snippet(code: String, prelude: String, extern_crates: Vec<String>, wild_crates: Vec<String>) -> String {
    fn indent_line(line: &str) -> String {
        format!("    {}\n",line)
    }
    let mut prefix = prelude;
    let mut body = String::new();
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
    run().unwrap();
}}
        ",prefix,body)

}

const USAGE: &str = "
Compile and run small Rust snippets
  -s, --static build statically (default is dynamic)
  -O, --optimize optimized static build
  -e, --expression evaluate an expression
  -i, --iterator evaluate an iterator
  -n, --lines evaluate expression over stdin; 'line' is defined
  -x, --extern... (string) add an extern crate to the snippet
  -X, --wild... (string) like -x but implies wildcard use

  Cache Management:
  --create (string...) initialize the static cache with crates
  --add  (string...) add new crates to the cache (after --create)
  --edit  edit the static cache Cargo.toml
  --build rebuild the static cache
  --doc  display documentation
  --edit-prelude edit the default prelude for snippets
  --alias (string...) crate aliases in form alias=crate_name (used with -x)

  Dynamic compilation:
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)
  --cfg... (string) pass configuration variables to rustc
  --libc  link dynamically against libc (special case)

  <program> (string) Rust program, snippet or expression
  <args> (string...) arguments to pass to program
";

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
            let docs = static_cache.join("target/doc/static_cache/index.html");
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
        let (crate_path,crate_name) = if file.exists() {
            let filename = crate_utils::path_file_name(&file);
            if file.is_dir() { // assumed to be Cargo directory
                if ! file.join("Cargo.toml").exists() {
                    args.quit(&format!("not a Cargo project directory: {}",file.display()));
                }
                (file.join("src/lib.rs"), filename)
            } else { // should be just a Rust source file
                if file.extension().or_die("expecting extension") != "rs" {
                    args.quit("expecting Rust source file");
                }
                let name = crate_utils::path_file_name(&file.with_extension(""));
                (file, name)
            }
        } else {
            (crate_utils::cache_path(&first_arg).join("src/lib.rs"), first_arg)
        };
        if args.get_bool("crate-path") {
            println!("{}",crate_utils::cache_path(&crate_name).display());
        } else {
            let valid_crate_name = crate_name.replace('-',"_");
            let cache = get_cache(false, false);
            let mut builder = process::Command::new("rustc");
                builder.args(&["-C","prefer-dynamic"]).args(&["-C","debuginfo=0"])
                .arg("-L").arg(&cache)
                .args(&["--crate-type","dylib"])
                .arg("--out-dir").arg(&cache)
                .arg("--crate-name").arg(&valid_crate_name)
                .arg(crate_path);
           for c in args.get_strings("cfg") {
                builder.arg("--cfg").arg(&c);
           }
           if args.get_bool("libc") {
                builder.arg("--extern").arg(&format!("libc={}/liblibc.{}",cache.display(),SO));
           }
           builder.status().or_die("can't run rustc");
        }
        return;
    }

    let build_static = args.get_bool("static");
    let optimize = args.get_bool("optimize");

    // we'll pass rest of arguments to program
    let program_args = args.get_strings("args");

    let cache = get_cache(build_static, optimize);

    let mut snippet = false;
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
        snippet = true;
        es::read_to_string(&file)
    };

    // proper Rust programs are accepted (this is a bit rough)
    if code.find("fn main").is_none() {
        let mut extern_crates = args.get_strings("extern");
        let wild_crates = args.get_strings("wild");
        if wild_crates.len() > 0 {
            extern_crates.extend(wild_crates.iter().cloned());
        }
        code = massage_snippet(code,prelude, extern_crates, wild_crates);
    }

    // we are going to put the expanded source and resulting exe in the runner bin dir
    let mut out_file = runner_directory().join("bin");
    if snippet {
        out_file.push(&file);
        out_file.set_extension("rs");
    } else {
        out_file.push("tmp.rs");
    }
    let mut program = out_file.clone();
    program.set_extension(EXE);

    es::write_all(&out_file,&code);

    let mut builder = process::Command::new("rustc");
    if ! build_static {
        builder.args(&["-C","prefer-dynamic"]).args(&["-C","debuginfo=0"]);
    } else {
        builder.arg(if optimize {"-O"} else {"-g"});
    }
    builder.arg("-L").arg(&cache);
    let status = builder.arg("-o").arg(&program)
        .arg(&out_file)
        .status().or_die("can't run rustc");
    if ! status.success() {
        return;
    }

    let mut builder = process::Command::new(&program);
    if ! build_static {
        builder.env("LD_LIBRARY_PATH",format!("{}:{}",rustup_lib(),cache.display()));
    }
    builder.args(&program_args)
        .status()
        .or_die(&format!("can't run program {:?}",program));

}


