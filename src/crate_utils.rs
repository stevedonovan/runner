use anyhow::{bail, Context, Result};
use dirs;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use toml;

static RUSTUP_LIB_VALUE: OnceLock<String> = OnceLock::new();

pub fn rustup_lib() -> Result<&'static str> {
    if RUSTUP_LIB_VALUE.get().is_none() {
        let output = std::process::Command::new("rustc")
            .args(&["--print", "target-libdir"])
            .output()
            .context("cannot query rustc target libdir")?;
        if !output.status.success() {
            bail!("rustc --print target-libdir failed");
        }
        let value = String::from_utf8(output.stdout)
            .context("rustc target libdir was not valid UTF-8")?
            .trim()
            .to_string();
        let _ = RUSTUP_LIB_VALUE.set(value);
    }
    Ok(RUSTUP_LIB_VALUE
        .get()
        .expect("rustup lib path should be initialized"))
}

pub fn is_unstable_toolchain() -> Result<bool> {
    Ok(rustup_lib()?.contains("nightly"))
}

pub fn proper_crate_name(crate_name: &str) -> String {
    crate_name.replace('-', "_")
}

pub fn plain_name(name: &str) -> bool {
    name.find(|c: char| c == '/' || c == '\\' || c == '.')
        .is_none()
}

pub fn path_file_name(p: &Path) -> String {
    if let Some(file_name) = p.file_name() {
        file_name.to_string_lossy().to_string()
    } else if let Ok(full_path) = p.canonicalize() {
        path_file_name(&full_path)
    } else {
        p.to_string_lossy().to_string()
    }
}

pub fn cargo_home() -> Result<PathBuf> {
    if let Ok(home) = env::var("CARGO_HOME") {
        // set in cargo runs
        Ok(home.into())
    } else {
        Ok(dirs::home_dir().context("no home!")?.join(".cargo"))
    }
}

pub fn cargo_dir(dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let mut path = dir.to_path_buf();
    let mut ok = true;
    while ok {
        let cargo_toml = path.join("Cargo.toml");
        if cargo_toml.exists() {
            return Ok((path, cargo_toml));
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
pub fn crate_info(cargo_toml: &Path) -> Result<CrateInfo> {
    let body = fs::read_to_string(cargo_toml).context("cannot read Cargo.toml")?;
    let toml = body
        .parse::<toml::Value>()
        .context("cannot parse Cargo.toml")?;
    let package = toml.as_table().unwrap().get("package").unwrap();
    let name = package.get("name").unwrap().as_str().unwrap().to_string();
    let edition = match package.get("edition") {
        None => "2015",
        Some(e) => e.as_str().unwrap(),
    }
    .to_string();
    Ok(CrateInfo { name, edition })
}
