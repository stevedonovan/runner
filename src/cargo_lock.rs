use crate::fatal::OrDie;
use std::fs;
use std::path::Path;
use toml;

#[derive(serde::Deserialize)]
pub struct CargoLock {
    pub package: Vec<Package>,
}

#[derive(serde::Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub dependencies: Option<Vec<String>>,
}

pub fn read_cargo_lock(path: &Path) -> CargoLock {
    let lockf = path.join("Cargo.lock");
    let body = fs::read_to_string(&lockf).or_die("cannot read Cargo.lock");
    toml::from_str(&body).or_die("can't deserialize")
}
