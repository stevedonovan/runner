use std::path::Path;
use super::es;

#[cfg(target_os = "windows")]
pub const EXE: &str = "exe";

#[cfg(not(target_os = "windows"))]
pub const EXE: &str = "";


#[cfg(target_os = "windows")]
pub const OPEN: &str = "start";

#[cfg(target_os = "macos")]
pub const OPEN: &str = "open";

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const OPEN: &str = "xdg-open";

pub fn open(p: &Path) {
    es::shell(&format!("{} {:?}",OPEN,p));
}


