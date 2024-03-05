use std::process::Command;
// use std::io::{self, Write};
let output = Command::new("rustc") // "echo $(rustc --print=sysroot)")
    .arg("--print=sysroot")
    .output()
    .expect("failed to execute process");

println!("status: {}", output.status);
// io::stdout().write_all(&output.stdout).unwrap();
// io::stderr().write_all(&output.stderr).unwrap();
println!("stdout={:?}", &output.stdout);
eprintln!("stderr={:?}", &output.stderr);
let Ok(str) = String::from_utf8(output.stdout) else { panic!("Maybe TODO")};
println!("str={str}");

assert!(output.status.success());
