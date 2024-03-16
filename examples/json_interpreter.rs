//: --static
use serde::de::Deserialize;
use serde_json::json;

let mut json = String::new();
io::stdin().lock().read_to_string(&mut json)?;

println!("{}",json!(json));
