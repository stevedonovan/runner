//: --static
use serde_json::json;

println!(
    "{}",
    json!({
        "code": 200,
        "success": true,
        "payload": {
            "features": [
                "awesome",
                "easyAPI",
                "lowLearningCurve"
            ]
        }
    })
);
