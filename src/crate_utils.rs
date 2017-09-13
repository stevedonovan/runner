// Find Cargo's source cache directory for a crate
use super::es;
use es::traits::*;
use std::env;
use std::path;

pub fn rustup_lib() -> String {
    es::shell("rustc --print sysroot") + "/lib"
}
pub fn proper_crate_name(crate_name: &str) -> String {
    crate_name.replace('-',"_")
}

pub fn path_file_name(p: &path::Path) -> String {
    if let Some(file_name) = p.file_name() {
        file_name.to_string_lossy().to_string()
    } else
    if let Ok(full_path) = p.canonicalize() {
        path_file_name(&full_path)
    } else {
        p.to_string_lossy().to_string()
    }
}

// there's a Crate for This...
fn semver_i (s: &str) -> u64 {
    let v = s.split('.').filter_map(|s| s.parse::<u64>().ok()).to_vec();
    (((v[0] << 8) + v[1]) << 8) + v[2]
}

pub fn cargo_home() -> path::PathBuf {
    env::var("CARGO_HOME") // set in cargo runs
        .unwrap_or(env::var("HOME").or_die("no home!") + "/.cargo").into()
}

pub fn cache_path(crate_name: &str) -> path::PathBuf {
    let home = cargo_home();
    let crate_root = path::PathBuf::from(home.join("registry/src"));
    // actual crate source is in some fairly arbitrary subdirectory of this
    let mut crate_dir = crate_root.clone();
    crate_dir.push(es::files(&crate_root).next().or_die("no crate cache directory"));

    let mut crates = Vec::new();
    for (p,d) in es::paths(&crate_dir) {
        if ! d.is_dir() { continue; }
        let filename = path_file_name(&p);
        if let Some(endc) = filename.rfind('-') {
            if &filename[0..endc] == crate_name {
                crates.push((p,semver_i(&filename[endc+1..])));
            }
        }
    }
    // crate versions in ascending order by semver rules
    crates.sort_by(|a,b| a.1.cmp(&b.1));
    crates.pop().or_die(&format!("no such crate: {}",crate_name)).0
}
pub fn crate_path(file: &path::Path, first_arg: &str) -> Result<(path::PathBuf,String),String> {
    if file.exists() {
        let filename = path_file_name(file);
        if file.is_dir() { // assumed to be Cargo directory
            if ! file.join("Cargo.toml").exists() {
                return Err(format!("not a Cargo project directory: {}",file.display()));
            }
            Ok((file.join("src/lib.rs"), filename))
        } else { // should be just a Rust source file
            if file.extension().or_die("expecting extension") != "rs" {
                return Err("expecting Rust source file".into());
            }
            let name = path_file_name(&file.with_extension(""));
            Ok((file.to_path_buf(), name))
        }
    } else {
        Ok((cache_path(first_arg).join("src/lib.rs"), first_arg.to_string()))
    }        
}
