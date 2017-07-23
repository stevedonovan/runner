# Running Little Rust Snippets

Cargo is a good, reliable way to build programs and libraries in Rust with versioned dependencies.
Those who have worked with the Wild West practices of C++ development find this particularly soothing,
and it's frequently given as one of the strengths of the Rust ecosystem.

However, it's not intended to make running little test programs straightforward - you have to
create a project with all the dependencies you wish to play with, and then edit `src/main.rs` and
do `cargo run`. A useful tip is to create a `src/bin` directory containing your little programs
and then use `cargo run --bin NAME` to run them. But there is a better way; if you have such
a project (say called 'cache') then the following compiler invocation will compile and link
a program against those dependencies:

```
$ rustc -L /path/to/cache/target/debug/deps mytest.rs
```
Of course, you need to manually run `cargo build` on your `cache` project whenever new dependencies
are added, or when the compiler is updated.

The `runner` tool helps to automate this pattern. It also supports _snippets_, which
are little 'scripts' formatted like Rust documentation examples.

```
$ cat print.rs
println!("Hello, World!")

$ runner print.rs
Hello, World!
```
It adds the necessary boilerplate and creates a proper Rust program in `temp`,
together with a editable prelude, which is initially:

```rust
#![allow(unused_imports)]
#![allow(dead_code)]
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::env;
use std::path::{PathBuf,Path};
#[allow(unused_macros)]
macro_rules! debug {
    ($x:expr) => {
        println!(\"{} = {:?}\",stringify!($x),$x);
    }
}
```

After first invocation of `runner`, this is found in `~/.cargo/.runner/prelude`.

`debug!` saves typing: `debug!(my_var)` is equivalent to `println!("my_var = {:?}",my_var)`.

As you can see, `runner` is very much about playing with small code snippets. By
_default_ it links the snippet _dynamically_ which is significantly faster. This
hello-world snippet takes 0.337s to build on my machine, but building statically with
`runner -s print.rs` takes 0.545s.

In both cases, the executable is `temp/print` - the dynamically-linked version can't
be run standalone unless you make the Rust runtime available globally.

However, the static option is much more flexible. You can easily create a static
cache with some common crates:

```
$ runner -c 'time json regex'
```

You can add as many crates if you like - number of available dependencies doesn't
slow down the linker. Thereafter, you may refer to these crates in snippets:

```rust
// json.rs
extern crate json;

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
```

And then build statically and run (any extra arguments are passed to the program.)

```json
$ runner -s json.rs
{"code":200,"success":true,"payload":{"features":["awesome","easyAPI","lowLearningCurve"]}}
```
You can use `?` instead of the ubiquitous and awful `unwrap`, since the boilerplate
encloses code in a function that returns `Result<(),Box<Error>>` - compatible with
any error return.

`runner` provides various utilities for managing the static cache:

```
$ runner --help
Compile and run small Rust snippets
  -s, --static build statically (default is dynamic)
  -O, --optimize optimized static build
  -c, --create (string...) initialize the static cache with crates
  -e, --edit  edit the static cache
  -b, --build rebuild the static cache
  -d, --doc  display
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)
  <program> (string) Rust program or snippet
  <args> (string...) arguments to pass to program
```

You can say `runner -e` to edit the static cache `Cargo.toml`, and `runner -b` to
rebuild the cache afterwards. The cache is built for both debug and release mode,
so using `-sO` you can build snippets in release mode. Documentation is also built
for the cache, and `runner -d` will open that documentation in the browser. (It's
always nice to have local docs, especially in bandwidth-starved situations.)

It would be good to provide such an experience for the dynamic-link case, since
it is faster. There is in fact a dynamic cache as well but support for linking
against external crates dynamically is very basic. It works fine for crates that
don't have any external depdendencies, e.g. this creates a `libjson.so` in the
dynamic cache:

```
$ runner -C json
```

But anything more complicated is hard;  dynamic linking is not a priority for
Rust tooling at the moment, and does not support it well enough without terrible
hacking and use of unstable features.





