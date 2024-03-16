use std::collections::HashSet;

// TODO use me
#[allow(dead_code)]
pub(crate) struct Snippet<'a> {
    a_gt2015: bool,
    b_verbose: bool,
    body_prelude: &'a str,
    code: &'a str,
    extern_crates: Vec<String>,
    macro_crates: &'a HashSet<String>,
    prelude: String,
    wild_crates: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn massage_snippet(
    code: &str,
    prelude: String,
    extern_crates: Vec<String>,
    wild_crates: Vec<String>,
    macro_crates: &HashSet<String>,
    prepend: &str,
    gt2015: bool,
    verbose: bool,
) -> (String, Vec<String>) {
    use crate::cache;
    use crate::strutil::{after, split, word_after};

    fn indent_line(line: &str) -> String {
        format!("    {line}\n")
    }

    let mut prefix = prelude;
    let mut crate_begin = String::new();
    let mut body = String::new();
    let mut deduced_externs = Vec::new();

    body += &prepend;
    if !extern_crates.is_empty() {
        let aliases = cache::get_aliases();
        for c in &extern_crates {
            prefix += &if let Some(aliased) = aliases.get(c) {
                format!("extern crate {aliased} as {c};\n",)
            } else {
                let mac = if macro_crates.contains(c) {
                    "#[macro_use] "
                } else {
                    ""
                };
                format!("{mac}extern crate {c};\n")
            };
        }
        for c in wild_crates {
            prefix += &format!("use {c}::*;\n");
        }
    }
    let mut lines = code.lines();
    let mut first = true;
    for line in lines.by_ref() {
        let line = line.trim_start();
        if first {
            // files may start with #! shebang or comment...
            if line.starts_with("#!/") || line.starts_with("//") {
                continue;
            }
            first = false;
        }
        // crate import, use should go at the top.
        // Particularly need to force crate-level attributes to the top
        // These must not be in the `run` function we're generating
        if let Some(rest) = after(line, "#[macro_use") {
            if let Some(ref crate_name) = word_after(rest, "extern crate ") {
                deduced_externs.push(crate_name.clone());
            }
            prefix += line;
            prefix.push('\n');
        } else if line.starts_with("extern ") || line.starts_with("use ") {
            if let Some(crate_name) = word_after(line, "extern crate ") {
                deduced_externs.push(crate_name.clone());
            }
            if gt2015 {
                if let Some(path) = word_after(line, "use ") {
                    let (name, rest) = split(&path, ':');
                    if !["std", "core", "alloc", "crate"].contains(&name) || rest.is_empty() {
                        deduced_externs.push(name.into());
                    }
                }
            }
            prefix += line;
            prefix.push('\n');
        } else if line.starts_with("#![") {
            // inner attributes really need to be at the top of the file
            crate_begin += line;
            crate_begin.push('\n');
        } else if !line.is_empty() {
            body += &indent_line(line);
            break;
        }
    }
    // and indent the rest!
    body.extend(lines.map(indent_line));

    // Add a final semicolon if there appears to be one missing
    // match body.trim_end().chars().last() {
    //     Some(';') | Some('}') => (),
    //     Some(_) => {
    //         eprintln!("adding a concluding semicolon to snippet to be safer");
    //         body += ";"
    //     }
    //     None => (),
    // }
    if verbose {
        eprintln!("body={body}");
    };
    deduced_externs.extend(extern_crates);
    deduced_externs.sort();
    deduced_externs.dedup();

    let massaged_code = format!(
        "{crate_begin}
{prefix}

fn run(args: Vec<String>) -> std::result::Result<(),Box<dyn std::error::Error+Sync+Send>> {{
{body}    Ok(())
}}

fn main() {{
    if let Err(e) = run(std::env::args().collect()) {{
        println!(\"error: {{:?}}\",e);
    }}
}}
"
    );

    if verbose {
        eprintln!("massaged_code={massaged_code}, deduced_externs={deduced_externs:?}");
    }
    (massaged_code, deduced_externs)
}
