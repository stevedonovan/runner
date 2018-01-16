// parse output of cargo build --message-format json,
// caching the results. Can get the exact name of the .rlib
// for the latest available version in the static cache.
extern crate json;
use std::path::{Path,PathBuf};
use std::fs::File;
use std::io::Write;

use super::es;
use es::traits::*;
use super::crate_utils::proper_crate_name;

use semver::Version;

fn as_str(v: &json::JsonValue) -> &str {
    v.as_str().unwrap()
}

fn read_entry(line: &str) -> Option<(String,String,Version,String,String,String)> {
    use strutil::next_2;

    if let Ok(doc) = json::parse(line) {
        let features = doc["features"].members().map(as_str).join(' ');
        let filenames = &doc["filenames"][0];
        if ! filenames.is_string() {
            println!("note: no filenames {}",line);
            return None;
        }
        let path = Path::new(as_str(filenames));

        let filename = path.file_name().unwrap();
        let ext = path.extension();
        if ! (ext.is_none() || ext.unwrap() == "exe") { // ignore build artifacts
            // package_id has version
            let package_id = as_str(&doc["package_id"]).split_whitespace();
            let (package,vs) = next_2(package_id);

            // but look for _crate name_ in name field
            let name = as_str(&doc["target"]["name"]);

            // get the cached source path
            let path = Path::new(as_str(&doc["target"]["src_path"]));

            let vs = Version::parse(vs).or_die("bad semver");
            let filename = filename.to_str().or_die("filename not valid Unicode");
            let src_path = path.to_str().or_die("cached path not valid Unicode");
            Some((package.into(),name.into(),vs,features,filename.into(),src_path.into()))
        } else {
            None
        }
    } else {
        None
    }
 }

fn file_name(cache: &Path) -> PathBuf {
    cache.join("cargo.meta")
}

#[derive(Debug)]
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
    entries: Vec<MetaEntry>
}

impl Meta {

    pub fn new() -> Meta {
        Meta {
            entries: Vec::new()
        }
    }

    pub fn exists(cache: &Path) -> bool {
        file_name(cache).exists()
    }

    pub fn new_from_file(cache: &Path) -> Meta {

        fn opt_field(fields: &[&str], idx: usize) -> String {
            if idx >= fields.len() {
                ""
            } else {
                fields[idx]
            }.into()
        }

        let mut v = Vec::new();
        let meta_f = file_name(cache);
        for line in es::lines(es::open(&meta_f)) {
            let parts = line.split(',').to_vec();
            v.push(MetaEntry{
                package: parts[0].into(),
                crate_name: parts[1].into(),
                version: Version::parse(parts[2]).unwrap(),
                features: parts[3].into(),
                debug_name: parts[4].into(),
                release_name: parts[5].into(),
                path: PathBuf::from(opt_field(&parts,6)),
            });
        }
        Meta {
            entries: v
        }
    }

    pub fn get_meta_entry<'a>(&'a self, name: &str) -> Option<&'a MetaEntry> {
        let mut v = self.entries.iter()
            .filter(|e| e.package == name || e.crate_name == name)
            .to_vec();
        if v.len() == 0 {
            return None;
        }
        if v.len() > 1 { // sort by semver in ascending order...
            v.sort_by(|a,b| a.version.cmp(&b.version));
        }
        Some(v[v.len()-1])
    }

    pub fn get_full_crate_name(&self, name: &str, debug: bool) -> Option<String> {
        self.get_meta_entry(name)
            .map(|e| if debug {e.debug_name.clone()} else {e.release_name.clone()})
    }

    pub fn dump_crates (&mut self, maybe_name: Option<String>) {
        if let Some(name) = maybe_name {
            if let Some(e) = self.get_meta_entry(&name) {
                println!("{}\t{}",e.crate_name,e.version);
            } else {
                es::quit("no such crate");
            }
        } else {
            self.entries.sort_by(|a,b| a.crate_name.cmp(&b.crate_name));
            for e in self.entries.iter() {
                println!("{}\t{}",e.crate_name,e.version);
            }
        }
    }

    // constructing from output of 'cargo build'

    pub fn debug(&mut self, txt: String) {
        for line in txt.lines() {
            // note that features is in form '"foo","bar"' which we
            // store as 'foo bar'
            if let Some((package,crate_name,vs,features,filename,path)) = read_entry(line) {
                let crate_name = proper_crate_name(&crate_name);
                self.entries.push(MetaEntry{
                    package: package,
                    crate_name: crate_name,
                    version: vs,
                    features: features,
                    debug_name: filename,
                    release_name: String::new(),
                    path: PathBuf::from(path),
                });
            }
        }
    }

    pub fn release(&mut self, txt: String) {
        for line in txt.lines() {
            if let Some((name,_,vs,_,filename,_)) = read_entry(line) {
                if let Some(entry) = self.entries.iter_mut()
                    .find(|e| e.package == name && e.version == vs) {
                        entry.release_name = filename;
                } else {
                    eprintln!("cannot find {} in release build",name);
                }
            }
        }
    }

    pub fn update(self, cache: &Path) {
        let meta_f = file_name(cache);
        let mut f = File::create(&meta_f).or_die("cannot create cargo.meta");
        for e in self.entries {
            write!(f,"{},{},{},{},{},{},{}\n",
                e.package,e.crate_name,e.version,e.features,
                e.debug_name,e.release_name,
                e.path.display()
            ).or_die("i/o?");
        }
    }

}
