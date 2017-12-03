// Find Cargo's source cache directory for a crate
use super::es;
use std::fs;
use es::traits::*;
use std::env;
use std::path::{Path,PathBuf};
use std::collections::HashMap;

lazy_static! {
    pub static ref RUSTUP_LIB: String = shell("rustc --print sysroot") + "/lib";
    pub static ref UNSTABLE: bool = RUSTUP_LIB.find("stable").is_none();
}

pub fn shell(cmd: &str) -> String {
    let o = ::std::process::Command::new(if cfg!(windows) {"cmd.exe"} else {"sh"})
     .arg(if cfg!(windows) {"/c"} else {"-c"})
     .arg(&format!("{} 2>&1",cmd))
     .output()
     .expect("failed to execute shell");
    String::from_utf8(o.stdout).expect("not UTF-8 output").trim_right_matches('\n').to_string()
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

// there's a Crate for This...
fn semver_i (s: &str) -> u64 {
    let v = s.split('.').filter_map(|s| s.parse::<u64>().ok()).to_vec();
    (((v[0] << 8) + v[1]) << 8) + v[2]
}

pub fn cargo_home() -> PathBuf {
    if let Ok(home) = env::var("CARGO_HOME") { // set in cargo runs
        home.into()
    } else {
        env::home_dir().or_die("no home!").join(".cargo")
    }
}

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
                crates.push((p,semver_i(&filename[endc+1..])));
            }
        }
    }
    // crate versions in ascending order by semver rules
    crates.sort_by(|a,b| a.1.cmp(&b.1));
    crates.pop().or_die(&format!("no such crate: {}",crate_name)).0
}

// Very hacky stuff - we want the ACTUAL crate name, not the project name
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

pub fn full_crate_name(deps: &Path, crate_name: &str) -> Option<String> {
    let mut res = Vec::new();
    let patt = format!("lib{}-",crate_name);
    for entry in fs::read_dir(deps).or_die("cannot access dependencies dir") {
        let entry = entry.or_die("cannot access deps entry");
        let path = entry.path();
        if let Some(f) = path.file_name() {
            let name = f.to_string_lossy();
            //println!("got {}",name);
            if name.starts_with(&patt) {
                let cname = &name[3..(name.len()-5) ];
                res.push(cname.to_string());
            }
        }
    }
    res.pop()
}

pub type Crates = HashMap<String,Vec<PathBuf>>;

pub fn get_cache_deps(deps: &Path) ->  Crates {
    let mut res: Crates = HashMap::new();
    for entry in fs::read_dir(deps).or_die("cannot access dependencies dir") {
        let entry = entry.or_die("cannot access deps entry");
        let path = entry.path();
        if let Some(f) = path.file_name() {
            let rlib = f.to_string_lossy();
            // pull out the crate name
            if let Some(idx) = rlib.rfind('-') {
                let name = &rlib[3..idx];
                let v = res.entry(name.into())
                    .or_insert_with(|| Vec::new());
                v.push(path.clone());
            }
        }
    }
    res
}

pub fn remove_duplicates(crates: Crates, do_clean: bool) {
    for (name,paths) in crates {
        if paths.len() > 1 {
            // Sort the paths in ascending time of modification
            let mut mpaths: Vec<_> = paths.into_iter()
                .map(|p| {
                    let time = p.metadata().unwrap().modified().unwrap();
                    (p,time)
                }).collect();
            mpaths.sort_by(|a,b| a.1.cmp(&b.1));

            // ignore the latest, and delete the rest

            if do_clean {
                mpaths.pop();
                println!("crate {} removing {} items",name,mpaths.len());
                for (p,_) in mpaths {
                    fs::remove_file(&p).expect("can't remove rlib");
                }
            } else {
                println!("crate {} has {} items",name,mpaths.len());
                for (p,_) in mpaths {
                    println!("{:?}",p);
                }
            }
        }
    }
}

pub fn remove_duplicate_cache_deps(deps: &Path, do_clean: bool) {
    let deps = get_cache_deps(deps);
    remove_duplicates(deps,do_clean);
}

fn inside_quotes(s: &str) -> String {
    s.chars()
        .skip_while(|&c| c != '"').skip(1)
        .take_while(|&c| c != '"').collect()
}


pub fn show_deps(stat_cache: &Path) {
    let cargo_lock = stat_cache.join("Cargo.lock");

    let f = es::open(cargo_lock);
    let res = es::lines(f)
        .skip_while(|line| ! line.starts_with("dependencies"))
        .skip(1)
        .take_while(|line| ! line.starts_with(']'))
        .map(|line| {
           let line = inside_quotes(&line);
           let toks = line.split_whitespace().take(2).to_vec();
           (toks[0].to_string(), toks[1].to_string())
         })
        .to_vec();

    for (name,vs) in res {
        println!("{} - {}",name,vs);
    }
}
