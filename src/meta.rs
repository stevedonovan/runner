// parse output of cargo build --message-format json,
// caching the results. Can get the exact name of the .rlib
// for the latest available version in the static cache.
use std::path::{Path,PathBuf};
use std::fs::File;
use std::io::Write;

use super::es;
use es::traits::*;
use super::crate_utils::proper_crate_name;

use semver::Version;

fn read_entry(txt: &str) -> Option<(&str,Version,&str,&str)> {
    use strutil::*;
    // features field
    if let Some(s) = after(txt,"features\":[") {
        let idx = s.find(']').unwrap();
        let features = &s[0..idx];

        // filenames field _usually_ single path
        let s = after(&s[idx..],"\"filenames\":[\"").unwrap();
        let endq = s.find('"').unwrap();
        let path = &s[0..endq];
        let slashp = path.rfind('/').unwrap();
        let filename = &path[slashp+1..];
        let idx = filename.find('.').unwrap_or(filename.len()-1);
        let ext = &filename[idx+1..];
        if ! (ext == "" || ext == "exe") { // ignore build artifacts
            // package_id has both name and version
            let s = after(&s[endq+1..],"package_id\":\"").unwrap();
            let (name,vs) = next_2(s.split_whitespace());
            let vs = Version::parse(vs).or_die("bad semver");
            Some((name,vs,features,filename))
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
struct MetaEntry {
    package: String,
    crate_name: String,
    version: Version,
    features: String,
    debug_name: String,
    release_name: String,
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

    pub fn new_from_file(cache: &Path) -> Meta {
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
            });
        }
        Meta {
            entries: v
        }
    }

    fn get_meta_entry<'a>(&'a self, name: &str) -> Option<&'a MetaEntry> {
        let mut v = self.entries.iter().filter(|e| e.crate_name == name).to_vec();
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
            if let Some((name,vs,features,filename)) = read_entry(line) {
                let package = name.to_string();
                let crate_name = proper_crate_name(&package);
                self.entries.push(MetaEntry{
                    package: package,
                    crate_name: crate_name,
                    version: vs,
                    features: features.replace(','," ").replace('"',""),
                    debug_name: filename.into(),
                    release_name: String::new()
                });
            }
        }
    }

    pub fn release(&mut self, txt: String) {
        for line in txt.lines() {
            if let Some((name,vs,_,filename)) = read_entry(line) {
                if let Some(entry) = self.entries.iter_mut()
                    .find(|e| e.package == name && e.version == vs) {
                        entry.release_name = filename.into();
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
            write!(f,"{},{},{},{},{},{}\n",
                e.package,e.crate_name,e.version,e.features,e.debug_name,e.release_name).or_die("i/o?");
        }
    }

}
