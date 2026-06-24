// parse output of cargo build --message-format json,
// caching the results. Can get the exact name of the .rlib
// for the latest available version in the static cache.
extern crate json;
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use super::crate_utils::proper_crate_name;
use crate::cache::static_cache_dir;
use crate::cargo_lock;

use semver::Version;
use serde::{Deserialize, Serialize};

fn as_str(v: &json::JsonValue) -> &str {
    v.as_str().unwrap()
}

fn read_entry(line: &str) -> Result<Option<MetaEntry>> {
    if let Ok(doc) = json::parse(line) {
        let features = doc["features"]
            .members()
            .map(as_str)
            .collect::<Vec<_>>()
            .join(" ");
        let filenames = &doc["filenames"][0];
        if !filenames.is_string() {
            return Ok(None);
        }
        let path = Path::new(as_str(filenames));

        let filename = path.file_name().unwrap();
        let ext = path.extension();
        if !(ext.is_none() || ext.unwrap() == "exe") {
            // ignore build artifacts
            // package_id has version
            let package_id = as_str(&doc["package_id"]).split('@').collect::<Vec<_>>();
            let (package, vs) = (package_id[0], package_id[1]);
            let idx = package.find('#').unwrap();
            let package = &package[idx + 1..];

            // but look for _crate name_ in name field
            let name = as_str(&doc["target"]["name"]);

            // get the cached source path
            let path = Path::new(as_str(&doc["target"]["src_path"]));

            let vs = Version::parse(vs).context("bad semver")?;
            let filename = filename.to_str().context("filename not valid Unicode")?;
            Ok(Some(MetaEntry {
                package: package.to_string(),
                crate_name: name.to_string(),
                version: vs,
                features,
                debug_name: filename.to_string(),
                release_name: "".to_string(),
                path: path.to_path_buf(),
            }))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

fn file_name(cache: &Path) -> PathBuf {
    cache.join("cargo.meta")
}

#[derive(Debug, Serialize, Deserialize)]
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
        file_name(cache).exists()
    }

    pub fn new_from_file(cache: &Path) -> Result<Meta> {
        use csv;
        let meta_f = file_name(cache);

        let mut rdr = csv::Reader::from_reader(io::BufReader::new(File::open(&meta_f)?));

        let mut v = Vec::new();

        for result in rdr.deserialize() {
            let record: MetaEntry = result?;
            v.push(record);
        }
        Ok(Meta { entries: v })
    }
    pub fn get_meta_entries(&self, name: &str) -> Vec<&MetaEntry> {
        self.entries
            .iter()
            .filter(|e| e.package == name || e.crate_name == name)
            .collect()
    }

    pub fn get_meta_entry(&self, name: &str) -> Option<&MetaEntry> {
        let mut v = self.get_meta_entries(name);
        if v.len() == 0 {
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
        entries.len() > 0
    }

    pub fn dump_crates(&mut self, maybe_names: Vec<String>, verbose: bool) -> Result<()> {
        if maybe_names.len() > 0 {
            let packages = if verbose {
                Some(cargo_lock::read_cargo_lock(&static_cache_dir()?)?.package)
            } else {
                None
            };
            for name in maybe_names {
                let entries = self.get_meta_entries(&name);
                if entries.len() > 0 {
                    for e in entries {
                        println!("{} = \"{}\"", e.package, e.version);
                        if let Some(ref packages) = packages {
                            let version = e.version.to_string();
                            print_dependencies(&e.package, &version, &packages, 1)?;
                        }
                    }
                } else {
                    bail!("no such crate {:?}", name);
                }
            }
        } else {
            self.entries.sort_by(|a, b| a.package.cmp(&b.package));
            for e in self.entries.iter() {
                println!("{} = \"{}\"", e.package, e.version);
            }
        }
        Ok(())
    }

    // constructing from output of 'cargo build'

    pub fn debug(&mut self, txt: String) -> Result<()> {
        for line in txt.lines() {
            // note that features is in form '"foo","bar"' which we
            // store as 'foo bar'
            if let Some(mut entry) = read_entry(line)? {
                entry.crate_name = proper_crate_name(&entry.crate_name);
                self.entries.push(entry);
            }
        }
        Ok(())
    }

    pub fn release(&mut self, txt: String) -> Result<()> {
        for line in txt.lines() {
            if let Some(new_entry) = read_entry(line)? {
                if let Some(entry) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.package == new_entry.package && e.version == new_entry.version)
                {
                    // we assume that there has been a debug build, which has filled in the debug_name.
                    entry.release_name = new_entry.debug_name;
                } else {
                    eprintln!("cannot find {} in release build", new_entry.package);
                }
            }
        }
        Ok(())
    }

    pub fn update(self, cache: &Path) -> Result<()> {
        use csv;
        let meta_f = file_name(cache);
        let mut wtr = csv::Writer::from_writer(io::BufWriter::new(File::create(&meta_f)?));
        for e in self.entries {
            wtr.serialize(e)?;
        }
        wtr.flush()?;
        Ok(())
    }
}

fn print_dependencies(
    package: &str,
    version: &str,
    packages: &[cargo_lock::Package],
    indent: u32,
) -> Result<()> {
    let p = packages
        .iter()
        .find(|p| p.name == package && p.version == version)
        .context("cannot find package in static cache Cargo.lock")?;
    let indents = (0..indent).map(|_| '\t').collect::<String>();
    if let Some(ref deps) = p.dependencies {
        for d in deps.iter() {
            let mut iter = d.split_whitespace();
            let package_name = iter.next().unwrap();
            let version = iter.next().unwrap();
            println!("{}{} = \"{}\"", indents, package_name, version);
            print_dependencies(package_name, version, packages, indent + 1)?;
        }
    }
    Ok(())
}
