// takes basic functionality from open crate
// and fills in the important _edit_ case, respecting POSIX
// and some Windows/MacOS limitations.
use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use std::process::Command;
extern crate open;

pub fn open(p: &Path) -> Result<()> {
    open::that(p).context("cannot open")?;
    Ok(())
}

pub fn edit(p: &Path) -> Result<()> {
    // Respect POSIX
    let editor = if let Ok(ed) = env::var("VISUAL") {
        ed
    } else if let Ok(ed) = env::var("EDITOR") {
        ed
    } else
    // MacOS open will NOT open random text files, so vim it is...
    if cfg!(target_os = "macos") {
        "vim".into()
    } else if cfg!(target_os = "windows") {
        // likewise, regular 'start' won't cope with files-without-known-extensions
        "notepad".into()
    } else {
        "open".into()
    };
    if editor == "open" {
        open(p)?;
    } else {
        Command::new(&editor)
            .arg(&p)
            .status()
            .with_context(|| format!("Cannot find editor {:?}: ", p))?;
    }
    Ok(())
}
