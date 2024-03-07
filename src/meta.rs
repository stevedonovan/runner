/// Parse output of cargo build --message-format json,
/// caching the results. Can get the exact name of the .rlib
/// for the latest available version in the static cache.
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::crate_utils::proper_crate_name;
use crate::cache::static_cache_dir;
use crate::cargo_lock;

use es::quit;
use es::traits::{Die, ToVec};

use semver::Version;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Target {
    kind: Vec<String>,
    crate_types: Vec<String>,
    name: String,
    src_path: String,
    edition: String,
    doc: bool,
    doctest: bool,
    test: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Profile {
    opt_level: String,
    debuginfo: u32,
    debug_assertions: bool,
    overflow_checks: bool,
    test: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code, clippy::struct_field_names)]
struct Package {
    reason: String,
    package_id: Option<String>,
    manifest_path: Option<String>,
    target: Option<Target>,
    profile: Option<Profile>,
    features: Option<Vec<String>>,
    filenames: Option<Vec<String>>,
    executable: Option<String>,
    fresh: Option<bool>,
    success: Option<bool>,
}

fn read_entry(line: &str) -> Option<(String, String, Version, String, String, String)> {
    // eprintln!("\nline={line}");
    use crate::strutil::next_2;

    let from_str = serde_json::from_str(line);
    if from_str.is_err() {
        eprintln!("\nline={line}");
        eprintln!("\nfrom_str: {from_str:?}");
    }

    let package: Package = from_str.ok()?;
    let features = package.features?.join(" ");

    let filenames = package.filenames?;
    let first_filename = &filenames[0];
    let path = Path::new(&first_filename);
    let filename = path.file_name().unwrap();

    let ext = path.extension();
    if ext.is_none() || ext.unwrap() == "exe" {
        return None;
    }

    // ignore build artifacts
    // package_id has version
    let package_ids = package.package_id?;
    let (package_id, vs) = next_2(package_ids.split_whitespace());

    // but look for _crate name_ in name field
    let target = package.target.or_die("no target found");

    let name = target.name;

    // get the cached source path
    // let path = Path::new(as_str(&doc["target"]["src_path"]));
    let src_path = target.src_path;
    let path = Path::new(&src_path);

    let vs = Version::parse(vs).or_die("bad semver");
    let filename = filename.to_str().or_die("filename not valid Unicode");
    let src_path = path.to_str().or_die("cached path not valid Unicode");

    Some((
        package_id.into(),
        name,
        vs,
        features,
        filename.into(),
        src_path.into(),
    ))
}

fn file_name(cache: &Path) -> PathBuf {
    cache.join("cargo.meta")
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct MetaEntry {
    pub package: String,
    pub crate_name: String,
    pub version: Version,
    pub features: String,
    debug_name: String,
    release_name: String,
    pub path: PathBuf,
}

pub struct Meta {
    entries: Vec<MetaEntry>,
}

impl Meta {
    pub fn new() -> Meta {
        Meta {
            entries: Vec::new(),
        }
    }

    pub fn exists(cache: &Path) -> bool {
        eprintln!("cache={cache:?}");
        file_name(cache).exists()
    }

    pub fn new_from_file(cache: &Path) -> Meta {
        fn opt_field(fields: &[&str], idx: usize) -> String {
            if idx >= fields.len() { "" } else { fields[idx] }.into()
        }

        let mut v = Vec::new();
        let meta_f = file_name(cache);
        let contents = fs::read_to_string(meta_f).or_die("cannot read metafile");
        for line in contents.lines() {
            let parts = line.split(',').to_vec();
            v.push(MetaEntry {
                package: parts[0].into(),
                crate_name: parts[1].into(),
                version: Version::parse(parts[2]).unwrap(),
                features: parts[3].into(),
                debug_name: parts[4].into(),
                release_name: parts[5].into(),
                path: PathBuf::from(opt_field(&parts, 6)),
            });
        }
        Meta { entries: v }
    }

    pub fn get_meta_entries<'a>(&'a self, name: &str) -> Vec<&'a MetaEntry> {
        self.entries
            .iter()
            .filter(|e| e.package == name || e.crate_name == name)
            .collect()
    }

    pub fn get_meta_entry<'a>(&'a self, name: &str) -> Option<&'a MetaEntry> {
        let mut v = self.get_meta_entries(name);
        if v.is_empty() {
            return None;
        }
        if v.len() > 1 {
            // sort by semver in ascending order...
            v.sort_by(|a, b| a.version.cmp(&b.version));
        }
        Some(v[v.len() - 1])
    }

    pub fn get_full_crate_name(&self, name: &str, debug: bool) -> Option<String> {
        self.get_meta_entry(name).map(|e| {
            if debug {
                e.debug_name.clone()
            } else {
                e.release_name.clone()
            }
        })
    }

    pub fn is_crate_present(&self, name: &str) -> bool {
        let entries = self.get_meta_entries(name);
        !entries.is_empty()
    }

    pub fn dump_crates(&mut self, maybe_names: Vec<String>, verbose: bool) {
        if maybe_names.is_empty() {
            self.entries
                .sort_by(|a, b| a.package.cmp(&b.package).then(a.version.cmp(&b.version)));
            self.entries
                .dedup_by(|a, b| a.package.eq(&b.package) && (a.version.eq(&b.version)));
            for e in &self.entries {
                println!("{} = \"{}\"", e.package, e.version);
            }
        } else {
            let packages = if verbose {
                Some(cargo_lock::read(&static_cache_dir()).package)
            } else {
                None
            };
            for name in maybe_names {
                let mut entries = self.get_meta_entries(&name);
                if entries.is_empty() {
                    quit(&format!("no such crate {name}"));
                }
                entries.sort_by(|a, b| a.package.cmp(&b.package).then(a.version.cmp(&b.version)));
                entries.dedup_by(|a, b| a.package.eq(&b.package) && (a.version.eq(&b.version)));
                for e in entries {
                    println!("{} = \"{}\"", e.package, e.version);
                    if let Some(ref packages) = packages {
                        let version = e.version.to_string();
                        print_dependencies(&e.package, &version, packages, 1);
                    }
                }
            }
        }
    }

    // constructing from output of 'cargo build'

    pub fn debug(&mut self, txt: &str) {
        for line in txt.lines() {
            // note that features is in form '"foo","bar"' which we
            // store as 'foo bar'
            if let Some((package, crate_name, vs, features, filename, path)) = read_entry(line) {
                let crate_name = proper_crate_name(&crate_name);
                self.entries.push(MetaEntry {
                    package,
                    crate_name,
                    version: vs,
                    features,
                    debug_name: filename,
                    release_name: String::new(),
                    path: PathBuf::from(path),
                });
            }
        }
    }

    pub fn release(&mut self, txt: &str) {
        for line in txt.lines() {
            if let Some((name, _, vs, _, filename, _)) = read_entry(line) {
                if let Some(entry) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.package == name && e.version == vs)
                {
                    entry.release_name = filename;
                } else {
                    eprintln!("cannot find {name} in release build");
                }
            }
        }
    }

    pub fn update(self, cache: &Path) {
        let meta_f = file_name(cache);
        let mut f = File::create(meta_f).or_die("cannot create cargo.meta");
        for e in self.entries {
            writeln!(
                f,
                "{},{},{},{},{},{},{}",
                e.package,
                e.crate_name,
                e.version,
                e.features,
                e.debug_name,
                e.release_name,
                e.path.display()
            )
            .or_die("i/o?");
        }
    }
}

fn print_dependencies(package: &str, version: &str, packages: &[cargo_lock::Package], indent: u32) {
    let p = packages
        .iter()
        .find(|p| p.name == package && p.version == version)
        .or_die("cannot find package in static cache Cargo.lock");
    let indents = (0..indent).map(|_| '\t').collect::<String>();
    if let Some(ref deps) = p.dependencies {
        for d in deps {
            let mut iter = d.split_whitespace();
            let Some(pname) = iter.next() else {
                continue;
            };
            let Some(version) = iter.next() else {
                continue;
            };
            println!("{indents}{pname} = \"{version}\"");
            print_dependencies(pname, version, packages, indent + 1);
        }
    }
}
