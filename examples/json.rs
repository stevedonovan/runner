// json.rs
use json;

let parsed = json::parse(r#"

{
    "code": 200,
    "success": true,
    "payload": {
        "features": [
            "awesome",
            "easyAPI",
            "lowLearningCurve"
        ]
    }
}

"#)?;

println!("{}",parsed);
