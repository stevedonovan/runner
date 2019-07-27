// cache management

use es::traits::*;
use std::process;
use std::env;
use std::fs;
use std::path::{Path,PathBuf};
use std::collections::HashMap;
use std::io::Write;

use crate::crate_utils;
use crate::meta;

use crate_utils::UNSTABLE;

use crate::state::State;

const STATIC_CACHE: &str = "static-cache";
const DYNAMIC_CACHE: &str = "dy-cache";

// this will be initially written to ~/.cargo/.runner/prelude and
// can then be edited.
const PRELUDE: &str = "
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
use std::collections::HashMap;

macro_rules! debug {
    ($x:expr) => {
        println!(\"{} = {:?}\",stringify!($x),$x);
    }
}
";

// a fairly arbitrary set of crates to start the ball rolling
// cf. https://github.com/brson/stdx
const KITCHEN_SINK: &str = "
    chrono
    regex
    serde_json
    serde_yaml
";

// Windows shell quoting is a mess, so we make single quotes
// become double quotes in expressions
pub fn quote(s: String) -> String {
    if cfg!(windows) {
        s.replace("\'","\"")
    } else {
        s
    }
}

pub fn runner_directory() -> PathBuf {
    let mut runner = crate_utils::cargo_home().join(".runner");
    if *UNSTABLE {
        runner.push("unstable");
    }
    runner
}

pub fn cargo(args: &[&str]) -> bool {
    let res = process::Command::new("cargo")
        .args(args)
        .status()
        .or_die("can't run cargo");
    res.success()
}

pub fn cargo_build(release: bool) -> Option<String> {
    use process::Stdio;
    use std::io::BufReader;
    use std::io::prelude::*;

    let mut c = process::Command::new("cargo");
    c.arg("build");
    if release {
        c.arg("--release");
    }
    c.stdout(Stdio::piped());
    c.arg("--message-format").arg("json");

    let mut res = c.spawn().or_die("can't run cargo");

    // collect all JSON records, and let the rest
    // pass through...
    let inb = BufReader::new(res.stdout.take().unwrap());
    let mut out = String::new();
    for line in inb.lines() {
        if let Ok(line) = line {
            if line.starts_with('{') {
                out += &line;
                out.push('\n');
            } else {
                println!("{}",line);
            }
        }
    }

    if res.wait().or_die("cargo build error").success() {
        Some(out)
    } else {
        None
    }
}

pub fn static_cache_dir() -> PathBuf {
    runner_directory().join(STATIC_CACHE)
}

pub fn get_metadata() -> meta::Meta {
    let static_cache = static_cache_dir();
    if meta::Meta::exists(&static_cache) {
        meta::Meta::new_from_file(&static_cache)
    } else {
        es::quit("please build the static cache with `runner --add <crate>...` first");
    }
}

pub fn static_cache_dir_check() -> PathBuf {
    let static_cache = static_cache_dir();
    if ! static_cache.exists() {
        es::quit("please build the static cache with `runner --add <crate>...` first");
    }
    static_cache
}

pub fn build_static_cache() -> bool {
    use crate::meta::*;
    let mut m = Meta::new();
    match cargo_build(false) {
        None => return false,
        Some(s) => m.debug(s)
    }
    match cargo_build(true) {
        None => return false,
        Some(s) => m.release(s)
    }
    m.update(&static_cache_dir());
    cargo(&["doc"])
}

pub fn create_static_cache(crates: &[String]) {
    use std::io::prelude::*;

    let static_cache = static_cache_dir();
    let exists = static_cache.exists();

    let crates = if crates.len() == 1 && crates[0] == "kitchen-sink" {
        KITCHEN_SINK.split_whitespace().map(|s| s.into()).collect()
    } else {
        crates.to_vec()
    };    

    // there are three forms possible
    // a plain crate name - we assume latest version ('*')
    // a name=vs - we'll ensure it gets quoted properly
    // a local Cargo project
    let crates_vs = crates.iter().map(|c| {
        if let Some(idx) = c.find('=') {
            // help with a little bit of quoting...
            let (name,vs) = (&c[0..idx], &c[(idx+1)..]);
            (name.to_string(),vs.to_string(),true)
        } else {
            if let Some((name,path)) = maybe_cargo_dir(&c) {
                // hello - this is a local Cargo project!
                (name, path.to_str().unwrap().to_string(),false)
            } else { // latest version of crate
                (c.to_string(), '*'.to_string(),true)
            }
        }
    }).to_vec();

    let mut home = runner_directory();
    env::set_current_dir(&home).or_die("cannot change to home directory");
    if ! exists {
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
        for (name,vs,semver) in crates_vs {
            if semver {
                write!(deps,"{}=\"{}\"\n",name,vs)
            } else {
               write!(deps,"{}={{path=\"{}\"}}\n",name,vs)
            }.or_die("could not modify Cargo.toml");
        }
    }
    if ! build_static_cache() {
        println!("Error occurred - restoring Cargo.toml");
        fs::copy(&tmpfile,"Cargo.toml").or_die("cannot restore Cargo.toml");
    }
}

fn maybe_cargo_dir(name: &str) -> Option<(String,PathBuf)> {
    let path = Path::new(name);
    if ! path.exists() || ! path.is_dir() {
        return None;
    }
    let full_path = path.canonicalize().or_die("bad path, man!");
    if let Ok((full_path,cargo_toml)) = crate_utils::cargo_dir(&full_path) {
        let name = crate_utils::crate_name(&cargo_toml);
        Some((name,full_path))
    } else {
        None
    }
}

// this is always called first and has the important role to ensure that
// runner's directory structure is created properly.
pub fn get_prelude() -> String {
    let home = runner_directory();
    let pristine = ! home.is_dir();
    if pristine {
        fs::create_dir_all(&home).or_die("cannot create runner directory");
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

pub fn get_cache(state: &State) -> PathBuf {
    let mut home = runner_directory();
    if state.build_static {
        home.push(STATIC_CACHE);
        home.push("target");
        home.push(if state.optimize {"release"} else {"debug"});
        home.push("deps");
    } else {
        home.push(DYNAMIC_CACHE);
    };
    home
}

pub fn add_aliases(aliases: Vec<String>) {
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

pub fn get_aliases() -> HashMap<String,String> {
    let alias_file = runner_directory().join("alias");
    if ! alias_file.is_file() { return HashMap::new(); }
    es::lines(es::open(&alias_file))
      .filter_map(|s| s.split_at_delim('=').trim()) // split into (String,String)
      .to_map()
}

