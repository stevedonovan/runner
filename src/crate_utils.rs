
use super::es;
use std::env;
use std::path::{Path,PathBuf};

use es::traits::*;

lazy_static! {
    pub static ref RUSTUP_LIB: String = es::shell("rustc --print sysroot") + "/lib";
    pub static ref UNSTABLE: bool = RUSTUP_LIB.find("nightly").is_some();
}

pub fn proper_crate_name(crate_name: &str) -> String {
    crate_name.replace('-',"_")
}

pub fn plain_name(name: &str) -> bool {
    name.find(|c:char| c=='/' || c=='\\' || c=='.').is_none()
}

pub fn path_file_name(p: &Path) -> String {
    if let Some(file_name) = p.file_name() {
        file_name.to_string_lossy().to_string()
    } else
    if let Ok(full_path) = p.canonicalize() {
        path_file_name(&full_path)
    } else {
        p.to_string_lossy().to_string()
    }
}

pub fn cargo_home() -> PathBuf {
    if let Ok(home) = env::var("CARGO_HOME") { // set in cargo runs
        home.into()
    } else {
        env::home_dir().or_die("no home!").join(".cargo")
    }
}

pub fn cargo_dir(dir: &Path) -> Result<(PathBuf,PathBuf),String> {
    let mut path = dir.to_path_buf();
    let mut ok = true;
    while ok {
        let cargo_toml = path.join("Cargo.toml");
        if cargo_toml.exists() {
            return Ok((path,cargo_toml));
        }
        ok = path.pop();
    }
    Err("No Cargo project in this path".into())
}

// Very hacky stuff - we want the ACTUAL crate name, not the directory/repo name
// So look just past [package] and scrape the name...
pub fn crate_name(cargo_toml: &Path) -> String {
    let name_line = es::lines(es::open(cargo_toml))
        .skip_while(|line| line.trim() != "[package]")
        .skip(1)
        .skip_while(|line| ! line.starts_with("name "))
        .next().or_die("totally fked Cargo.toml");
    let idx = name_line.find('"').or_die("no name?");
    (&name_line[(idx+1)..(name_line.len()-1)]).into()
}


