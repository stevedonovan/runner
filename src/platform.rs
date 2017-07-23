use std::path::Path;
use std::env;
use std::process::Command;
use super::es::traits::*;
extern crate open;

#[cfg(target_os = "windows")]
pub const EXE: &str = "exe";

#[cfg(not(target_os = "windows"))]
pub const EXE: &str = "";


pub fn open(p: &Path) {
    open::that(p).or_die("cannot open");
}

pub fn edit(p: &Path) {
    let editor = if let Ok(ed) = env::var("VISUAL") {
        ed
    } else
    if let Ok(ed) = env::var("EDITOR") {
        ed
    } else
    if cfg!(target_os = "macos") {
        "emacs".into()
    } else {
        "open".into()
    };
    if editor == "open" {
        open(p);
    } else {
        Command::new(&editor).arg(&p).status().or_die("Cannot find editor");
    }
}


