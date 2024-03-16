//: --static
use serde::de::Deserialize;
use serde_json::Value;

let j = serde_json::from_str::<Value>(
    r#"{
    "code": 200,
    "dynamic": false,
    "reason": {
        "sadly": [
        "serde_json",
        "does",
        "not",
        "support",
        "dylibs",
        "🤷🏼‍♂️"
        ],
        "see" : "https://robert.kra.hn/posts/2022-09-09-speeding-up-incremental-rust-compilation-with-dylibs/#limitation-the-diamond-dependency-problem"
    }
    }"#
)
.unwrap();

debug!(j);
