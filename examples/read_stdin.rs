use std::io::{self, Read};

fn read_stdin() -> Result<String, io::Error> {
    let mut buffer = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    handle.read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn main() {
    println!("Type lines of text at the prompt and hit Ctrl-D when done");
    let content = read_stdin().expect("Problem reading input");
    println!("Read from stdin:\n{}", content);
}
