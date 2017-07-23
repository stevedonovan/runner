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

mod crate_utils;
mod platform;

use platform::{open,edit,EXE};

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
    let mut home = env::home_dir().or_die("no home!");
    home.push(".cargo");
    home.push(".runner");
    home
}

fn cargo(args: &[&str]) {
    let res = process::Command::new("cargo")
        .args(args)
        .status()
        .or_die("can't run cargo");
    if ! res.success() {
        es::quit("cargo failed");
    }
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

fn build_static_cache() {
    cargo(&["build"]);
    cargo(&["build","--release"]);
    cargo(&["doc"]);
}

fn create_static_cache(crates: &[String], create: bool) {
    use std::io::prelude::*;
    let mut home = runner_directory();
    env::set_current_dir(&home).or_die("cannot change to home directory");
    if create {
        cargo(&["new","--bin",STATIC_CACHE]);
    }
    home.push(STATIC_CACHE);
    env::set_current_dir(&home).or_die("could not create static cache");
    {
        let mut deps = fs::OpenOptions::new().append(true)
            .open("Cargo.toml").or_die("could not append to cargo.toml");
        for c in crates {
            write!(deps,"{}=\"*\"\n",c).or_die("could not modify cargo.toml");
        }
    }
    build_static_cache();
}

fn prelude_and_cache(build_static: bool, optimize: bool) -> (String,PathBuf) {
    let mut home = runner_directory();
    let pristine = ! home.is_dir();
    if pristine {
        fs::create_dir(&home).or_die("cannot create runner directory");
    }
    let mut prelude = home.clone();
    prelude.push("prelude");
    let scache;
    let cache = if build_static {
        let dir = if optimize {"release"} else {"debug"};
        scache = format!("{}/target/{}/deps",STATIC_CACHE,dir);
        &scache
    } else {DYNAMIC_CACHE};
    home.push(cache);
    if pristine {
        es::write_all(&prelude,PRELUDE);
        fs::create_dir(&home).or_die("cannot create dynamic cache");
    }
    (es::read_to_string(&prelude),home)
}

fn massage_snippet(code: String, prelude: String) -> String {
    fn indent_line(line: &str) -> String {
        format!("    {}\n",line)
    }
    let mut prefix = prelude;
    let mut body = String::new();
    {
        let mut lines = code.lines();
        for line in lines.by_ref() {
            let line = line.trim_left();
            if line.starts_with("//") || line.starts_with("#[") ||
                line.starts_with("extern ") || line.starts_with("use ") {
                prefix += line;
                prefix.push('\n');
            } else {
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
  -c, --create (string...) initialize the static cache with crates
  -a, --add  (string...) add new crates to the cache (after --create)
  -e, --edit  edit the static cache Cargo.toml
  -b, --build rebuild the static cache
  -d, --doc  display
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)
  <program> (string) Rust program or snippet
  <args> (string...) arguments to pass to program
";

fn main() {
    let args = lapp::parse_args(USAGE);

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

    let file = PathBuf::from(args.get_string("program"));

    if args.get_bool("crate-path") || args.get_bool("compile") {
        let (crate_path,crate_name) = if file.exists() {
            let filename = crate_utils::path_file_name(&file);
            (file, filename)
        } else {
            let arg = args.get_string("program");
            (crate_utils::cache_path(&arg), arg)
        };
        if args.get_bool("crate-path") {
            println!("{}",crate_path.display());
        } else {
            let valid_crate_name = crate_name.replace('-',"_");
            let (_,cache) = prelude_and_cache(false, false);
            process::Command::new("rustc")
                .args(&["-C","prefer-dynamic"]).args(&["-C","debuginfo=0"])
                .arg("-L").arg(&cache)
                .args(&["--crate-type","dylib"])
                .arg("--out-dir").arg(&cache)
                .arg("--crate-name").arg(&valid_crate_name)
                .arg(crate_path.join("src/lib.rs"))
           .status().or_die("can't run rustc");
        }
        return;
    }
    let ext = file.extension().or_die("no file extension");
    if ext != "rs" {
        es::quit("file extension must be .rs");
    }

    let build_static = args.get_bool("static");
    let optimize = args.get_bool("optimize");

    // we'll pass rest of arguments to program
    let args = args.get_strings("args");

    // we are going to put the expanded source and resulting exe in temp
    let out_dir = "temp";
    if ! fs::metadata(out_dir).is_dir() {
        fs::create_dir(out_dir).or_die("cannot create temp directory here");
    }

    let mut code = es::read_to_string(&file);

    let (prelude,cache) = prelude_and_cache(build_static, optimize);

    if code.find("fn main").is_none() {
        code = massage_snippet(code,prelude);
    }

    let mut out_file = PathBuf::from(out_dir);
    out_file.push(&file);
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
    builder.args(&args)
        .status()
        .or_die(&format!("can't run program {:?}",program));

}


