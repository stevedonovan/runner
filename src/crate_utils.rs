
use std::fs;
use std::env;
use std::path::{Path,PathBuf};
use toml;
use es::traits::*;
use dirs;

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
        dirs::home_dir().or_die("no home!").join(".cargo")
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

pub struct CrateInfo {
    pub name: String,
    pub edition: String,

}

// we want the ACTUAL crate name, not the directory/repo name
pub fn crate_info (cargo_toml: &Path) -> CrateInfo {
    let body = fs::read_to_string(cargo_toml).or_die("cannot read Cargo.toml");
    let toml = body.parse::<toml::Value>().or_die("cannot parse Cargo.toml");
    let package = toml.as_table().unwrap()
        .get("package").unwrap();
    let name = package.get("name").unwrap()
        .as_str().unwrap().to_string();
    let edition = match package.get("edition") {
        None => "2015",
        Some(e) => e.as_str().unwrap()
    }.to_string();
    CrateInfo {
        name, edition,
    }
}


