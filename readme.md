# Running Little Rust Snippets

## Leaving the Comfort (and Restrictions) of Cargo

Cargo is a good, reliable way to build programs and libraries in Rust with versioned dependencies.
Those who have worked with the Wild West practices of C++ development find this particularly soothing,
and it's frequently given as one of the strengths of the Rust ecosystem.

However, it's not intended to make running little test programs straightforward - you have to
create a project with all the dependencies you wish to play with, and then edit `src/main.rs` and
do `cargo run`. A useful tip is to create a `src/bin` directory containing your little programs
and then use `cargo run --bin NAME` to run them. But there is a better way; if you have such
a project (say called 'cache') then the following invocation will compile and link
a program against those dependencies (`rustc` is an unusually intelligent compiler)

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

You can even - on Unix platforms - add a 'shebang' line to invoke runner:

```
$ cat hello
#!/usr/bin/env runner
println!("Hello, World!");

$ ./hello
Hello, World!
```

`runner` adds the necessary boilerplate and creates a proper Rust program in `~/.cargo/.runner/bin`,
prefixed with a prelude, which is initially:

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

After first invocation of `runner`, this is found in `~/.cargo/.runner/prelude`;
you can edit it later with `runner --edit-prelude`.

`debug!` saves typing: `debug!(my_var)` is equivalent to `println!("my_var = {:?}",my_var)`.

## Adding External Crates

As you can see, `runner` is very much about playing with small code snippets. By
_default_ it links the snippet _dynamically_ which is significantly faster. This
hello-world snippet takes 0.34s to build on my machine, but building statically with
`runner -s print.rs` takes 0.55s.

In both cases, the executable goes into the same directory as the expanded code  - but
the dynamically-linked version can't be run standalone unless you make the Rust runtime
available globally.

The static option is much more flexible. You can easily create a static
cache with some common crates:

```
$ runner --create 'time json regex'
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
You can use `?` in snippets instead of the ubiquitous and awful `unwrap`, since the boilerplate
encloses code in a function that returns `Result<(),Box<Error>>` which is compatible with
any error return.

`runner` provides various utilities for managing the static cache:

```
$ runner -h
Compile and run small Rust snippets
  -s, --static build statically (default is dynamic)
  -O, --optimize optimized static build
  -e, --expression evaluate an expression
  -i, --iterator evaluate an iterator
  -n, --lines evaluate expression over stdin; 'line' is defined
  -x, --extern... (string) add an extern crate to the snippet
  -X, --wild... (string) like -x but implies wildcard use

  Cache Management:
  --create (string...) initialize the static cache with crates
  --add  (string...) add new crates to the cache (after --create)
  --edit  edit the static cache Cargo.toml
  --build rebuild the static cache
  --doc  display documentation
  --edit-prelude edit the default prelude for snippets
  --alias (string...) crate aliases in form alias=crate_name (used with -x)

  Dynamic compilation:
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)
  --cfg... (string) pass configuration variables to rustc
  --libc  link dynamically against libc (special case)

  <program> (string) Rust program, snippet or expression
  <args> (string...) arguments to pass to program

```

You can say `runner --edit` to edit the static cache `Cargo.toml`, and `runner --build` to
rebuild the cache afterwards. The cache is built for both debug and release mode,
so using `-sO` you can build snippets in release mode. Documentation is also built
for the cache, and `runner --doc` will open that documentation in the browser. (It's
always nice to have local docs, especially in bandwidth-starved situations.)

## Dynamic Linking

It would be good to provide such an experience for the dynamic-link case, since
it is faster. There is in fact a dynamic cache as well but support for linking
against external crates dynamically is very basic. It works fine for crates that
don't have any external depdendencies, e.g. this creates a `libjson.so` in the
dynamic cache:

```
$ runner -C json
```
And then you can run the `json.rs` example without `-s`.

The `--compile` action takes three kinds of arguments:

  - a crate name that is already loaded and known to Cargo
  - a Cargo directory
  - a Rust source file - the crate name is the file name without extension.

But anything more complicated is harder, because dynamic linking is not a priority for
Rust tooling at the moment. So we have to build more elaborate libraries without the
help of Cargo. (The following assumes that you have already brought in `regex` for a Cargo project,
so that the Cargo cache is populated, e.g. with `runner --add regex`)


```
runner -C --cfg 'feature="default"' --cfg 'feature="use_std"' libc
runner -C --libc memchr
runner -C --libc thread-id
runner -C --cfg 'feature="std"'  void
runner -C utf8-ranges
runner -C unreachable
runner -C aho-corasick
runner -C lazy_static
runner -C thread_local
runner -C regex-syntax
runner -C regex
```
This script drives home how tremendously irritating life in Rust would be without Cargo.
We have to track the dependencies, ensure that the correct default features are enabled in the
compilation, and special-case crates which directly link to `libc`.

However, the results feel worthwhile. Compiling the first `regex` documented example:

```rust
extern crate regex;
use regex::Regex;
let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
assert!(re.is_match("2014-01-01"));
```
With a static build (`-s`) I get 0.90s on this machine, and 0.47s with dynamic linking.
[macbook]

A useful trick - if you want to look at the `Cargo.toml` of an already downloaded crate
to find out dependencies and features, then this command will open it for you:

```
favorite-editor $(runner -P some-crate)/Cargo.toml
```

## Rust on the Command-line

There are a few Perl-inspired features. The `-e` flag compiles and evaluates an
_expression_.  You can use it as an unusually strict desktop calculator:

```
$ runner -e "10 + 20*4.5"
error[E0277]: the trait bound `{integer}: std::ops::Mul<{float}>` is not satisfied
  --> temp/tmp.rs:20:22
   |
20 |     let res = 10 + 20*4.5;
   |                      ^ no implementation for `{integer} * {float}`
```

Likewise, you have to say `1.2f64.sin()` because `1.2` has ambiguous type.

`--expression` is very useful if you quickly want to find out how Rust
will evaluate an expression - we do a debug print for maximum flexibility.

```
$ runner -e 'PathBuf::from("bonzo.dog").extension()'
Some("dog")
```

(This works because we have a `use std::path::PathBuf` in the runner prelude.)

`-i` (or `--iterator`) evaluates iterator expressions and does a debug
dump of the results:

```
$ runner -i '(0..5).map(|i| (10*i,100*i))'
(0, 0)
(10, 100)
(20, 200)
(30, 300)
(40, 400)
```

Any extra command-line arguments are available for these commands, so:

```
$ runner -i 'env::args().enumerate()' one 'two 2' 3
(0, "/home/steve/.cargo/.runner/bin/tmp")
(1, "one")
(2, "two 2")
(3, "3")
```

And finally `-n` (or `--lines`) evaluates the expression for each line in
standard input:

```
$ echo "hello there" | runner -n 'line.to_uppercase()'
"HELLO THERE"
```
The `-x` flag (`--extern`) allows you to insert an `extern crate` into your
snippet. This is particularly useful for these one-line shortcuts. For
example, my `easy-shortcuts` crate has a couple of helper functions:

```
$ runner -xeasy_shortcuts -e 'easy_shortcuts::argn_err(1,"gimme an arg!")' 'an arg'
"an arg"
$ runner -xeasy_shortcuts -e 'easy_shortcuts::argn_err(1,"gimme an arg!")'
/home/steve/.cargo/.runner/bin/tmp error: no argument 1: gimme an arg!
```
This also applies to `--iterator`:

```
$ runner -xeasy_shortcuts -i 'easy_shortcuts::files(".")'
"json.rs"
"print.rs"
```

With long crate names like this, you can define _aliases_:

```
$ runner --alias es=easy_shortcuts
$ runner -xes -e 'es::argn_err(1,"gimme an arg!")'
...
```
By default, `runner -e` does a dynamic link, and there are known limitations.
By also using `--static`, you can evaluate expressions against crates
compiled as static libraries. So, assuming that we have
`time` in the static cache (`runner --add time` will do that for you):

```
$ runner -s -xtime -e "time::now()"
Tm { tm_sec: 34, tm_min: 4, tm_hour: 9, tm_mday: 28, tm_mon: 6, tm_year: 117,
tm_wday: 5, tm_yday: 208, tm_isdst: 0, tm_utcoff: 7200, tm_nsec: 302755857 }
```

'-X' is like '-x' except it brings all the crate's symbols into scope.
Not something you would overdo in regular code, but it makes for shorter
command lines - the last example becomes (note how short flags can be combined):

```
$ runner -sXtime -e "now()"
...
```

If you can get away with dynamic linking, then `runner` can make it
easy to test a module interactively. In this way you get much of the
benefit of a fully interactive interpreter (a REPL):

```
$ cat universe.rs
pub fn answer() -> i32 {
    42
}
$ runner -C universe.rs
$ runner -xuniverse -e "universe::answer()"
42
```

