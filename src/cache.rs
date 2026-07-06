// cache management

use crate::meta;
use crate::{cache, crate_utils};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

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
    let mut runner = env::var("RUNNER_HOME")
        .map(PathBuf::from)
        .or_else(|_| crate_utils::cargo_home())?
        .join(".runner");
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

fn remove_file_and_log(file_path: &Path) {
    println!("removing {}", file_path.display());
    if let Err(e) = fs::remove_file(file_path) {
        println!("could not remove {}: {:?}", file_path.display(), e);
    }
}
pub fn build_static_cache() -> Result<bool> {
    use crate::meta::*;
    let old = get_metadata().unwrap_or_else(|_| Meta::new());
    let mut m = Meta::new();
    match cargo_build(true)? {
        None => return Ok(false),
        Some(s) => m.release(s), // passop
    }?;
    let deps = static_cache_dir()?
        .join("target")
        .join("release")
        .join("deps");
    for p in &m.entries {
        if let Some(q) = old.entries.iter().find(|e| e.package == p.package) {
            if q.version != p.version {
                // get rid of the old version!
                let old_rlib = &deps.join(&q.debug_name);
                let old_rmeta = &old_rlib.with_extension("rmeta");
                remove_file_and_log(&old_rlib);
                remove_file_and_log(&old_rmeta);
            }
        }
    }
    m.update(&static_cache_dir()?)?;
    cargo(&["doc"])
}

pub fn create_static_cache(crates: &[String]) -> Result<()> {
    let static_cache = static_cache_dir()?;
    let exists = static_cache.exists();

    let mut home = runner_directory()?;
    env::set_current_dir(&home).context("cannot change to home directory")?;
    if !exists {
        if !cargo(&["new", "--bin", STATIC_CACHE])? {
            bail!("cannot create static cache");
        }
    }

    home.push(STATIC_CACHE);
    env::set_current_dir(&home).context("cannot change to static cache directory")?;
    // there are three forms possible
    // a plain crate name - we assume latest version ('*')
    // a name=vs - we'll ensure it gets quoted properly
    // a local Cargo project
    for c in crates {
        if c.contains("=") {
            let c = c.replace("=", "@").to_string();
            cargo(&["add", c.as_str()])?;
        } else if let Some((_, path)) = maybe_cargo_dir(&c)? {
            // hello - this is a local Cargo project!
            cargo(&["add", "--path", path.to_str().unwrap()])?;
        } else {
            // latest version of crate
            cargo(&["add", c.as_str()])?;
        }
    }

    build_static_cache()?;
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

// assume that `program` always exists, but `exe` may not
pub fn compare_file_times(program: &Path, exe: &Path) -> Result<bool> {
    let meta1 = program.metadata()?;
    Ok(if let Ok(meta2) = exe.metadata() {
        meta1.modified()? > meta2.modified()?
    } else {
        true
    })
}

// Find scripts (and env.rs) on `RUNNER_PATH` if defined.
// If the original 'calling' script dir is available, we use that after checking current dir
// (this becomes available as `@SCRIPT` in `RUNNER_PATH`)
pub fn lookup_file_path(file: &str, script_path: Option<&PathBuf>) -> Option<PathBuf> {
    if let Ok(path) = env::var("RUNNER_PATH") {
        let path = if let Some(script_path) = script_path {
            let script_dir = script_path.parent()?;
            path.replace("@SCRIPT", &script_dir.display().to_string())
        } else {
            path
        };
        for p in path.split(':') {
            let candidate = Path::new(p).join(file);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    } else {
        let path = PathBuf::from(file);
        if path.is_file() {
            Some(path)
        } else if let Some(script_path) = script_path {
            let path = script_path.parent()?.join(file);
            if path.is_file() {
                Some(path)
            } else {
                None
            }
        } else {
            None
        }
    }
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
