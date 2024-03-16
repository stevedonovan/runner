use crate::cache;
use crate::crate_utils;
use crate::state::State;
use es::traits::Die;
use regex::Regex;

use lapp::Args;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Stdio;
use std::{fs, process};

fn simplify_qualified_names(text: &str) -> String {
    let std = "std::";
    let mut res = String::new();
    let mut s = text;
    while let Some(pos) = s.find(std) {
        res.push_str(&s[0..pos]);
        s = &s[pos + std.len()..];
        if let Some(pos) = s.find("::") {
            s = &s[pos + 2..];
        }
    }
    res.push_str(s);
    res
}

// handle two useful cases:
// - compile a crate as a dynamic library, given a name and an output dir
// - compile a program, given a program
pub(crate) fn dlib_or_prog(
    args: &lapp::Args,
    state: &State,
    crate_name: &str,
    crate_path: &Path,
    output_program: Option<&Path>,
    mut extern_crates: Vec<String>,
    features: Vec<String>,
) -> bool {
    let verbose = args.get_bool("verbose");
    let simplify = !args.get_bool("no-simplify");
    let debug = !state.optimize;

    // implicit linking works fine, until it doesn't
    extern_crates.extend(args.get_strings("extern"));
    extern_crates.sort();
    extern_crates.dedup();
    // libc is such a special case
    if args.get_bool("libc") {
        extern_crates.push("libc".into());
    }
    let mut cfg = args.get_strings("cfg");
    let explicit_features = args.get_strings("features");
    for f in if explicit_features.is_empty() {
        features
    } else {
        explicit_features
    } {
        cfg.push(format!("feature=\"{f}\""));
    }
    let cache = cache::get_cache(state);
    let mut builder = process::Command::new("rustc");
    if state.edition != "2015" {
        builder.args(["--edition", &state.edition]);
    }
    if state.build_static {
        // static build
        builder.arg(if state.optimize { "-O" } else { "-g" });
        if state.optimize {
            // no point in carrying around all that baggage...
            builder.args(["-C", "debuginfo=0"]);
        }
    } else {
        // stripped-down dynamic link
        builder
            .args(["-C", "prefer-dynamic"])
            .args(["-C", "debuginfo=0"]);
        if let Ok(link) = args.get_string_result("link") {
            if verbose {
                eprintln!("linking against {link}");
            }
            builder.arg("-L").arg(&link);
        }
    }
    // implicitly linking against crates in the dynamic or static cache
    builder.arg("-L").arg(&cache);
    if state.exe {
        builder.arg("-o").arg(output_program.unwrap());
    } else {
        // as a dynamic library
        builder
            .args(["--crate-type", "dylib"])
            .arg("--out-dir")
            .arg(&cache)
            .arg("--crate-name")
            .arg(&crate_utils::proper_crate_name(crate_name));
    }
    for c in cfg {
        builder.arg("--cfg").arg(&c);
    }

    // explicit --extern references require special treatment for
    // static builds, since the libnames include a hash.
    // So we look for the latest crate of this name

    let extern_crates: Vec<(String, String)> = if state.build_static && !extern_crates.is_empty() {
        let m = cache::get_metadata();
        extern_crates
            .into_iter()
            .map(|c| {
                (
                    m.get_full_crate_name(&c, debug)
                        .or_then_die(|_| format!("no such crate '{c}' in static cache: use --add")),
                    c,
                )
            })
            .collect()
    } else {
        extern_crates
            .into_iter()
            .map(|c| (format!("{DLL_PREFIX}{c}{DLL_SUFFIX}"), c))
            .collect()
    };

    for (name, c) in extern_crates {
        let full_path = PathBuf::from(&cache).join(&name);
        let ext = format!("{c}={}", full_path.display());
        if verbose {
            eprintln!("extern {ext}");
        }
        builder.arg("--extern").arg(&ext);
    }
    builder.arg(crate_path);
    // eprintln!("!!!simplify={simplify}");
    if simplify {
        if isatty::stderr_isatty() {
            builder.args(["--color", "always"]);
        }
        let output = builder.output().or_die("can't run rustc");
        let status = output.status.success();
        if !status {
            let err = String::from_utf8_lossy(&output.stderr);
            eprintln!("{}", simplify_qualified_names(&err));
            // eprintln!("original version:{}", err);
        }
        status
    } else {
        builder.status().or_die("can't run rustc").success()
    }
}

#[allow(clippy::too_many_arguments)]
// Compiles a program
pub fn program(
    b: impl Fn(&str) -> bool,
    program: &PathBuf,
    args: &Args<'_>,
    verbose: bool,
    state: &State,
    rust_path: &PathBuf,
    externs: Vec<String>,
    exe_suffix: &str,
) -> ControlFlow<()> {
    if b("run") {
        if !program.exists() {
            args.quit(&format!("program {program:?} does not exist"));
        }
    } else {
        if verbose {
            eprintln!("Building program ({program:?}) from source {rust_path:?}",);
            let mode_stem = if state.build_static { "stat" } else { "dynam" };
            eprintln!("Compiling {mode_stem}ically");
        };
        if !dlib_or_prog(
            args,
            state,
            "",
            rust_path,
            Some(program),
            externs,
            Vec::new(),
        ) {
            process::exit(1);
        }
        if verbose {
            println!("Compiled {rust_path:?} successfully to {program:?}");
        }
    }
    if b("compile-only") {
        // copy and return
        let file_name = rust_path.file_name().or_die("no file name?");
        let out_dir = args.get_path("output");
        let home = if out_dir == Path::new("cargo") {
            let home = crate_utils::cargo_home().join("bin");
            if !home.is_dir() {
                // With Windows, standalone installer does not create this directory
                // (may well be a Bugge)
                fs::create_dir(&home).or_die("could not create Cargo bin directory");
                println!(
                    "creating Cargo bin directory {}\nEnsure it is on your PATH",
                    home.display()
                );
            }
            home
        } else {
            out_dir
        };
        let here = home.join(file_name).with_extension(exe_suffix);
        println!("Copying {} to {}", program.display(), here.display());
        fs::copy(program, &here).or_die("cannot copy program");
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}

pub(crate) fn check_well_formed(verbose: bool, quoted_src: &String) -> bool {
    // First do a rough check for essential prerequisite: fn main()
    let re = Regex::new(r"(?x)fn\ main()").unwrap(); // (?x) accounts for extra whitespace

    let matches = re.find_iter(quoted_src).count();

    match matches {
        0 => {
            if verbose {
                eprintln!("source does not contain fn main(), thus a snippet");
            }
            return false;
        }
        1 => (),
        _ => es::quit(
            "Invalid source, contains {matches} occurrences of fn main(), at most 1 is allowed",
        ),
    };

    // Check if it's a valid program
    let source_code = quoted_src;

    // Get the home directory
    #[allow(deprecated)]
    let home_dir = std::env::home_dir().expect("Failed to get home directory");

    // Combine home directory with the relative path
    let mut output_path = PathBuf::from(".cargo/bin/metadata");
    output_path = home_dir.join(output_path);

    let mut rustc_process = Command::new("rustc")
        .args(["-o", output_path.to_str().unwrap(), "--emit=metadata", "-"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped()) // Capture stderr explicitly
        .spawn()
        .expect("Failed to spawn rustc process");

    let mut stdin = rustc_process
        .stdin
        .take()
        .expect("Failed to get stdin pipe");

    // Write the source code to the stdin pipe
    stdin
        .write_all(source_code.as_bytes())
        .expect("Failed to write to stdin pipe");

    // Close the stdin pipe to signal end of input
    drop(stdin);

    // Wait for the rustc process to finish and collect its output
    let output = rustc_process
        .wait_with_output()
        .expect("Failed to wait for rustc process");

    // Check for errors
    if output.status.success() {
        if verbose {
            eprintln!("rustc succeeded");
        }
        true
    } else {
        // "rustc failed with error:\n[{: >100}\n]",
        // String::from_utf8_lossy(&output.stderr).trim_end()
        if verbose {
            let mut indented_error = String::new();
            for line in String::from_utf8_lossy(&output.stderr).lines() {
                indented_error.push_str(&format!("    {line}\n"));
            }
            eprintln!(
                "snippet not well-formed: rustc check failed with error:\n[{}]\n",
                indented_error.trim_end() // Remove trailing newline
            );
        }
        false
    }
}
