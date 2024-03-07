// takes basic functionality from open crate
// and fills in the important _edit_ case, respecting POSIX
// and some Windows/MacOS limitations.
use super::es::traits::Die;
use std::env;
use std::path::Path;
use std::process::Command;
extern crate open;

pub fn open(p: &Path) {
    open::that(p).or_die("cannot open");
}

pub fn edit(p: &Path) {
    // Respect POSIX
    let editor = if let Ok(ed) = env::var("VISUAL") {
        ed
    } else if let Ok(ed) = env::var("EDITOR") {
        ed
    // MacOS open will NOT open random text files, so vim it is...
    } else if cfg!(target_os = "macos") {
        "vim".into()
    } else if cfg!(target_os = "windows") {
        // likewise, regular 'start' won't cope with files-without-known-extensions
        // Notepad is useless, so use Wordpad
        "write".into()
    } else {
        "open".into()
    };
    if editor == "open" {
        open(p);
    } else {
        Command::new(&editor)
            .arg(p)
            .status()
            .or_die(&format!("Cannot find editor {p:?}: "));
    }
}
