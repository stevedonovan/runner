#!/usr/bin/env cargo-eval
//! Dependencies can be specified in the script file itself as follows:
//!
//! ```cargo
//! [dependencies]
//! rand = "0.8.0"
//! ```

use rand::prelude::*;

fn main() {
    let x: u64 = random();
    println!("A random number: {}", x);
}
