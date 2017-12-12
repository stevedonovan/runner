// Find Cargo's source cache directory for a crate
use super::es;

use std::env;
use std::path::{Path,PathBuf};

use es::traits::*;
use semver::Version;

lazy_static! {
    pub static ref RUSTUP_LIB: String = es::shell("rustc --print sysroot") + "/lib";
    pub static ref UNSTABLE: bool = RUSTUP_LIB.find("stable").is_none();
}

pub fn proper_crate_name(crate_name: &str) -> String {
    crate_name.replace('-',"_")
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

// looks at the Cargo source cache and returns the directory
// of the _latest_ version of a crate.
pub fn cache_path(crate_name: &str) -> PathBuf {
    let home = cargo_home();
    let crate_root = PathBuf::from(home.join("registry/src"));
    // actual crate source is in some fairly arbitrary subdirectory of this
    let mut crate_dir = crate_root.clone();
    crate_dir.push(es::files(&crate_root).next().or_die("no crate cache directory"));

    let mut crates = Vec::new();
    for (p,d) in es::paths(&crate_dir) {
        if ! d.is_dir() { continue; }
        let filename = path_file_name(&p);
        if let Some(endc) = filename.rfind('-') {
            if &filename[0..endc] == crate_name {
                let vs = Version::parse(&filename[endc+1..]).or_die("bad semver");
                crates.push((p,vs));
            }
        }
    }
    // crate versions in ascending order by semver rules
    crates.sort_by(|a,b| a.1.cmp(&b.1));
    crates.pop().or_die(&format!("no such crate: {}",crate_name)).0
}

// Very hacky stuff - we want the ACTUAL crate name, not the directory/repo name
// So look just past [package] and scrape the name...
fn crate_name(cargo_toml: &Path) -> String {
    let name_line = es::lines(es::open(cargo_toml))
        .skip_while(|line| line.trim() != "[package]")
        .skip(1)
        .skip_while(|line| ! line.starts_with("name "))
        .next().or_die("totally fked Cargo.toml");
    let idx = name_line.find('"').or_die("no name?");
    (&name_line[(idx+1)..(name_line.len()-1)]).into()
}

pub fn crate_path(file: &Path, first_arg: &str) -> Result<(PathBuf,String),String> {
    if file.exists() {
        if file.is_dir() { // assumed to be Cargo directory
            let cargo_toml = file.join("Cargo.toml");
            if ! cargo_toml.exists() {
                return Err(format!("not a Cargo project directory: {}",file.display()));
            }
            Ok((file.join("src").join("lib.rs"), crate_name(&cargo_toml)))
        } else { // should be just a Rust source file
            if file.extension().or_die("expecting extension") != "rs" {
                return Err("expecting Rust source file".into());
            }
            let name = path_file_name(&file.with_extension(""));
            Ok((file.to_path_buf(), name))
        }
    } else {
        let project_dir = cache_path(first_arg);
        let cargo_toml = project_dir.join("Cargo.toml");
        Ok((project_dir.join("src").join("lib.rs"), crate_name(&cargo_toml)))
    }
}

