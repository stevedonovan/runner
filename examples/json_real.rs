//: --static
use serde::de::Deserialize;
use serde_json::Value;

println!("{}",
    serde_json::from_str::<Value>(
        r#"{
          "reason": "compiler-artifact",
          "package_id": "json 0.12.4 (git+https://github.com/maciejhirsz/json-rust#0775592d339002ab148185264970c2a6e30b5d37)",
          "manifest_path": "/Users/donf/.cargo/git/checkouts/json-rust-8fc837b9d242b7ce/0775592/Cargo.toml",
          "target": {
            "kind": [
              "lib"
            ],
            "crate_types": [
              "lib"
            ],
            "name": "json",
            "src_path": "/Users/donf/.cargo/git/checkouts/json-rust-8fc837b9d242b7ce/0775592/src/lib.rs",
            "edition": "2018",
            "doc": true,
            "doctest": true,
            "test": true
          },
          "profile": {
            "opt_level": "0",
            "debuginfo": 2,
            "debug_assertions": true,
            "overflow_checks": true,
            "test": false
          },
          "features": [],
          "filenames": [
            "/Users/donf/projects/runner/target/debug/deps/libjson-00a147c9a3e66787.rlib",
            "/Users/donf/projects/runner/target/debug/deps/libjson-00a147c9a3e66787.rmeta"
          ],
          "executable": null,
          "fresh": true
        }
"#
    )
    .unwrap()
);
