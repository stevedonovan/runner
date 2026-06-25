// cache management

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use crate::meta;
use crate::{cache, crate_utils};

use crate_utils::is_unstable_toolchain;

use crate::state::State;
use anyhow::{bail, Context, Result};

const STATIC_CACHE: &str = "static-cache";
const DYNAMIC_CACHE: &str = "dy-cache";

// this will be initially written to ~/.cargo/.runner/prelude and
// can then be edited.
const PRELUDE: &str = "
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_macros)]
use std::{fs,io,env};
use std::fs::File;
use std::io::prelude::*;
use std::path::{PathBuf,Path};
use std::collections::HashMap;
use std::time::Duration;
use std::thread;

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
    if cfg!(all(windows, not(feature = "no_quote_replacement"))) {
        s.replace("\'", "\"")
    } else {
        s
    }
}

pub fn runner_directory() -> Result<PathBuf> {
    let mut runner = crate_utils::cargo_home()?.join(".runner");
    if is_unstable_toolchain()? {
        runner.push("unstable");
    }
    Ok(runner)
}

pub fn cargo(args: &[&str]) -> Result<bool> {
    let res = process::Command::new("cargo")
        .args(args)
        .status()
        .context("can't run cargo")?;
    Ok(res.success())
}

pub fn cargo_build(release: bool) -> Result<Option<String>> {
    use process::Stdio;
    use std::io::prelude::*;
    use std::io::BufReader;

    let mut c = process::Command::new("cargo");
    c.arg("build");
    if release {
        c.arg("--release");
    }
    c.stdout(Stdio::piped());
    c.arg("--message-format").arg("json");

    let mut res = c.spawn().context("can't run cargo")?;

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
                println!("{}", line);
            }
        }
    }

    if res.wait().context("cargo build error")?.success() {
        Ok(Some(out))
    } else {
        Ok(None)
    }
}

pub fn static_cache_dir() -> Result<PathBuf> {
    Ok(runner_directory()?.join(STATIC_CACHE))
}

pub fn get_metadata() -> Result<meta::Meta> {
    let static_cache = static_cache_dir()?;
    if meta::Meta::exists(&static_cache) {
        meta::Meta::new_from_file(&static_cache)
    } else {
        bail!("please build the static cache with `runner --add <crate>...` first");
    }
}

pub fn static_cache_dir_check() -> Result<PathBuf> {
    let static_cache = static_cache_dir()?;
    if !static_cache.exists() {
        bail!("please build the static cache with `runner --add <crate>...` first");
    }
    Ok(static_cache)
}

pub fn build_static_cache() -> Result<bool> {
    use crate::meta::*;
    let mut m = Meta::new();
    match cargo_build(true)? {
        None => return Ok(false),
        Some(s) => m.release(s), // passop
    }?;
    m.update(&static_cache_dir()?)?;
    cargo(&["doc"])
}

pub fn create_static_cache(crates: &[String]) -> Result<()> {
    use std::io::prelude::*;

    let static_cache = static_cache_dir()?;
    let exists = static_cache.exists();

    let crates = if crates.len() == 1 && crates[0] == "kitchen-sink" {
        KITCHEN_SINK.split_whitespace().map(|s| s.into()).collect()
    } else {
        crates.to_vec()
    };

    let mut home = runner_directory()?;
    env::set_current_dir(&home).context("cannot change to home directory")?;

    let mdata = if !exists {
        if !cargo(&["new", "--bin", STATIC_CACHE])? {
            bail!("cannot create static cache");
        }
        None
    } else {
        Some(get_metadata()?)
    };
    let check_crate = |s: &str| {
        if let Some(m) = &mdata {
            m.is_crate_present(s)
        } else {
            false
        }
    };

    // there are three forms possible
    // a plain crate name - we assume latest version ('*')
    // a name=vs - we'll ensure it gets quoted properly
    // a local Cargo project
    let mut crates_vs = Vec::new();
    for c in &crates {
        if let Some(idx) = c.find('=') {
            // help with a little bit of quoting...
            let (name, vs) = (&c[0..idx], &c[(idx + 1)..]);
            crates_vs.push((name.to_string(), vs.to_string(), true));
        } else if let Some((name, path)) = maybe_cargo_dir(&c)? {
            // hello - this is a local Cargo project!
            if !check_crate(&name) {
                crates_vs.push((
                    name,
                    path.to_str()
                        .context("local Cargo path was not valid Unicode")?
                        .to_string(),
                    false,
                ));
            }
        } else if !check_crate(c) {
            // latest version of crate
            crates_vs.push((c.to_string(), '*'.to_string(), true));
        }
    }

    if crates_vs.len() == 0 {
        return Ok(());
    }

    home.push(STATIC_CACHE);
    env::set_current_dir(&home).context("could not change to static cache directory")?;
    let tmpfile = env::temp_dir().join("Cargo.toml");
    fs::copy("Cargo.toml", &tmpfile).context("cannot back up Cargo.toml")?;
    {
        let mut deps = fs::OpenOptions::new()
            .append(true)
            .open("Cargo.toml")
            .context("could not append to Cargo.toml")?;
        for (name, vs, semver) in crates_vs {
            if semver {
                write!(deps, "{}=\"{}\"\n", name, vs)
            } else {
                write!(deps, "{}={{path=\"{}\"}}\n", name, vs)
            }
            .context("could not modify Cargo.toml")?;
        }
    }
    if !build_static_cache()? {
        println!("Error occurred - restoring Cargo.toml");
        fs::copy(&tmpfile, "Cargo.toml").context("cannot restore Cargo.toml")?;
    }
    Ok(())
}

fn maybe_cargo_dir(name: &str) -> Result<Option<(String, PathBuf)>> {
    let path = Path::new(name);
    if !path.exists() || !path.is_dir() {
        return Ok(None);
    }
    let full_path = path.canonicalize().context("bad path")?;
    if let Ok((full_path, cargo_toml)) = crate_utils::cargo_dir(&full_path) {
        let name = crate_utils::crate_info(&cargo_toml)?.name;
        Ok(Some((name, full_path)))
    } else {
        Ok(None)
    }
}

// this is always called first and has the important role to ensure that
// runner's directory structure is created properly.
pub fn get_prelude() -> Result<String> {
    let home = runner_directory()?;
    let pristine = !home.is_dir();
    if pristine {
        fs::create_dir_all(&home).context("cannot create runner directory")?;
    }
    let prelude = home.join("prelude");
    let bin = home.join("bin");
    if pristine {
        fs::write(&prelude, PRELUDE).context("cannot write prelude")?;
        fs::create_dir(&home.join(DYNAMIC_CACHE)).context("cannot create dynamic cache")?;
    }
    if pristine || !bin.is_dir() {
        fs::create_dir(&bin).context("cannot create output directory")?;
    }
    fs::read_to_string(&prelude).context("cannot read prelude")
}

pub fn get_cache(state: &State) -> Result<PathBuf> {
    let mut home = runner_directory()?;
    if state.build_static {
        home.push(STATIC_CACHE);
        home.push("target");
        home.push(if state.optimize { "release" } else { "debug" });
        home.push("deps");
    } else {
        home.push(DYNAMIC_CACHE);
    };
    Ok(home)
}

pub fn add_aliases(aliases: Vec<String>) -> Result<()> {
    if aliases.len() == 0 {
        return Ok(());
    }
    let alias_file = runner_directory()?.join("alias");
    let mut f = if alias_file.is_file() {
        fs::OpenOptions::new().append(true).open(&alias_file)
    } else {
        fs::File::create(&alias_file)
    }
    .context("cannot open runner alias file")?;

    for crate_alias in aliases {
        write!(f, "{}\n", crate_alias).context("cannot write to runner alias file")?;
    }
    Ok(())
}

pub fn get_aliases() -> Result<HashMap<String, String>> {
    let alias_file = runner_directory()?.join("alias");
    if !alias_file.is_file() {
        return Ok(HashMap::new());
    }
    let contents = fs::read_to_string(&alias_file).context("cannot read alias file")?;
    Ok(contents
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect())
}

pub fn save_missing_crates(missing: &[String]) -> Result<()> {
    let p = file_missing_crates()?;
    fs::write(p, &missing.join("\n")).context("cannot write to cache directory")?;
    Ok(())
}

pub fn file_missing_crates() -> Result<PathBuf> {
    let p = cache::runner_directory()?.join("missing-crates");
    Ok(p)
}

pub fn delete_missing_crates() -> Result<()> {
    let p = file_missing_crates()?;
    fs::remove_file(&p).context("cannot delete missing crates")?;
    Ok(())
}

pub fn read_missing_crates() -> Result<Vec<String>> {
    let p = file_missing_crates()?;
    let missing = fs::read_to_string(&p).context("cannot read missing crates file")?;
    Ok(missing.split('\n').map(|s| s.to_string()).collect())
}
