//: -s
// filetime.rs
use std::fs;    // Demo uses 'runner -s --no-prelude filetime.rs' to show how to suppress this
use filetime::FileTime;

let metadata = fs::metadata("filetime.rs").unwrap();

let mtime = FileTime::from_last_modification_time(&metadata);
println!("{}", mtime);

let atime = FileTime::from_last_access_time(&metadata);
assert!(mtime < atime);

// Inspect values that can be interpreted across platforms
println!("{}", mtime.unix_seconds());
println!("{}", mtime.nanoseconds());

// Print the platform-specific value of seconds
println!("{}", mtime.seconds());
