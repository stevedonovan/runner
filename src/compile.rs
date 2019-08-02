
use lapp;
use es::traits::*;
use crate::crate_utils;
use crate::cache;
use crate::state::State;

use std::process;
use std::path::{Path,PathBuf};
use std::env::consts::{DLL_SUFFIX,DLL_PREFIX};
use std::collections::{HashSet};

fn simplify_qualified_names(text: &str) -> String {
    let std = "std::";
    let mut res = String::new();
    let mut s = text;
    while let Some(pos) = s.find(std) {
        res.push_str(&s[0..pos]);
        s = &s[pos+std.len()..];
        if let Some(pos) = s.find("::") {
            s = &s[pos+2..];
        }
    }
    res.push_str(s);
    res
}

// handle two useful cases:
// - compile a crate as a dynamic library, given a name and an output dir
// - compile a program, given a program
pub fn compile_crate(args: &lapp::Args, state: &State,
    crate_name: &str, crate_path: &Path,
    output_program: Option<&Path>, mut extern_crates: Vec<String>, features: Vec<String>) -> bool
{
    let verbose = args.get_bool("verbose");
    let simplify = ! args.get_bool("no-simplify");
    let debug = ! state.optimize;

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
    for f in if explicit_features.len() > 0 {explicit_features} else {features} {
        cfg.push(format!("feature=\"{}\"",f));
    }
    let cache = cache::get_cache(&state);
    let mut builder = process::Command::new("rustc");
    if state.edition != "2015" {
        builder.args(&["--edition",&state.edition]);
    }
    if ! state.build_static { // stripped-down dynamic link
        builder.args(&["-C","prefer-dynamic"]).args(&["-C","debuginfo=0"]);
        if let Ok(link) = args.get_string_result("link") {
            if verbose { println!("linking against {}",link); }
            builder.arg("-L").arg(&link);
        }
    } else { // static build
        builder.arg(if state.optimize {"-O"} else {"-g"});
        if state.optimize {
            // no point in carrying around all that baggage...
            builder.args(&["-C","debuginfo=0"]);
        }
    }
    // implicitly linking against crates in the dynamic or static cache
    builder.arg("-L").arg(&cache);
    if ! state.exe { // as a dynamic library
        builder.args(&["--crate-type","dylib"])
        .arg("--out-dir").arg(&cache)
        .arg("--crate-name").arg(&crate_utils::proper_crate_name(crate_name));
    } else {
        builder.arg("-o").arg(output_program.unwrap());
    }
    for c in cfg {
        builder.arg("--cfg").arg(&c);
    }

    // explicit --extern references require special treatment for
    // static builds, since the libnames include a hash.
    // So we look for the latest crate of this name

    let extern_crates: Vec<(String,String)> =
    if state.build_static && extern_crates.len() > 0 {
        let m = cache::get_metadata();
        extern_crates.into_iter().map(|c|
            (m.get_full_crate_name(&c,debug)
                .or_then_die(|_| format!("no such crate '{}' in static cache: use --add",c)),c)
        ).collect()
    } else {
        extern_crates.into_iter().map(|c|
            (format!("{}{}{}",DLL_PREFIX,c,DLL_SUFFIX),c)
        ).collect()
    };

    for (name,c) in extern_crates {
        let full_path = PathBuf::from(&cache).join(&name);
        let ext = format!("{}={}",c,full_path.display());
        if verbose {
            println!("extern {}",ext);
        }
        builder.arg("--extern").arg(&ext);
    }
    builder.arg(crate_path);
    if simplify {
        if isatty::stderr_isatty() {
            builder.args(&["--color","always"]);
        }
        let output = builder.output().or_die("can't run rustc");
        let status = output.status.success();
        if ! status {
            let err = String::from_utf8_lossy(&output.stderr);
            eprintln!("{}",simplify_qualified_names(&err));
        }
        status
    } else {
        builder.status().or_die("can't run rustc").success()
    }
}

pub fn massage_snippet(code: String, prelude: String,
        extern_crates: Vec<String>, wild_crates: Vec<String>, macro_crates: HashSet<String>, body_prelude: String, is2018: bool) -> (String,Vec<String>) {
    use crate::strutil::{after,word_after,split};

    fn indent_line(line: &str) -> String {
        format!("    {}\n",line)
    }

    let mut prefix = prelude;
    let mut crate_begin = String::new();
    let mut body = String::new();
    let mut deduced_externs = Vec::new();

    body += &body_prelude;
    if extern_crates.len() > 0 {
        let aliases = cache::get_aliases();
        for c in &extern_crates {
            prefix += &if let Some(aliased) = aliases.get(c) {
                format!("extern crate {} as {};\n",aliased,c)
            } else {
                let mac = if macro_crates.contains(c) {"#[macro_use] "} else {""};
                format!("{}extern crate {};\n",mac,c)
            };
        }
        for c in wild_crates {
            prefix += &format!("use {}::*;\n",c);
        }
    }
    let mut lines = code.lines();
    let mut first = true;
    for line in lines.by_ref() {
        let line = line.trim_start();
        if first { // files may start with #! shebang or comment...
            if line.starts_with("#!/") || line.starts_with("//") {
                continue;
            }
            first = false;
        }
        // crate import, use should go at the top.
        // Particularly need to force crate-level attributes to the top
        // These must not be in the `run` function we're generating
        if let Some(rest) = after(line,"#[macro_use") {
            if let Some(crate_name) = word_after(rest,"extern crate ") {
                deduced_externs.push(crate_name);
            }
            prefix += line;
            prefix.push('\n');
        } else
        if line.starts_with("extern ") || line.starts_with("use ") {
            if let Some(crate_name) = word_after(line,"extern crate ") {
                deduced_externs.push(crate_name);
            }
            if is2018 {
                if let Some(path) = word_after(line,"use ") {
                    let (name,rest) = split(&path,':');
                    if ! ["std","core","alloc","crate"].contains(&name) || rest == "" {
                        deduced_externs.push(name.into());
                    }
                }
            }
            prefix += line;
            prefix.push('\n');
        } else
        if line.starts_with("#![") {
            // inner attributes really need to be at the top of the file
            crate_begin += line;
            crate_begin.push('\n');
        } else
        if line.len() > 0 {
            body += &indent_line(line);
            break;
        }
    }
    // and indent the rest!
    body.extend(lines.map(indent_line));

    deduced_externs.extend(extern_crates);
    deduced_externs.sort();
    deduced_externs.dedup();

    let massaged_code = format!("{}
{}

fn run(args: Vec<String>) -> std::result::Result<(),Box<dyn std::error::Error+Sync+Send>> {{
{}    Ok(())
}}
fn main() {{
    if let Err(e) = run(std::env::args().collect()) {{
        println!(\"error: {{:?}}\",e);
    }}
}}
",crate_begin,prefix,body);

    (massaged_code, deduced_externs)

}

