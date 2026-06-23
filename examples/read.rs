// read.rs
let code = fs::read_to_string("read.rs")?;
//println!("bytes {}", code.len());
debug!(code.len());
