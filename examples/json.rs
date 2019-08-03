//: --static
use serde_json::json;

println!("{}",
    json! ({
        "hello": 42,
    })
);
