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

You can even - on Unix platforms - add a 'shebang' line to invoke runner:

```
$ cat hello
#!/home/steve/.cargo/bin/runner
println!("Hello, World!");

$ ./hello
Hello, World!
```

It adds the necessary boilerplate and creates a proper Rust program in `~/.cargo/.runner/bin`,
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

After first invocation of `runner`, this is found in `~/.cargo/.runner/prelude`.

`debug!` saves typing: `debug!(my_var)` is equivalent to `println!("my_var = {:?}",my_var)`.

As you can see, `runner` is very much about playing with small code snippets. By
_default_ it links the snippet _dynamically_ which is significantly faster. This
hello-world snippet takes 0.337s to build on my machine, but building statically with
`runner -s print.rs` takes 0.545s.

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
You can use `?` instead of the ubiquitous and awful `unwrap`, since the boilerplate
encloses code in a function that returns `Result<(),Box<Error>>` - compatible with
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

  Cache Management:
  --create (string...) initialize the static cache with crates
  --add  (string...) add new crates to the cache (after --create)
  --edit  edit the static cache Cargo.toml
  --build rebuild the static cache
  --doc  display
  --edit-prelude edit the default prelude for snippets
  --alias (string...) crate aliases in form alias=crate_name (used with -x)  

  Dynamic compilation:
  -P, --crate-path show path of crate source in Cargo cache
  -C, --compile  compile crate dynamically (limited)

  <program> (string) Rust program, snippet or expression
  <args> (string...) arguments to pass to program

```

You can say `runner --edit` to edit the static cache `Cargo.toml`, and `runner --build` to
rebuild the cache afterwards. The cache is built for both debug and release mode,
so using `-sO` you can build snippets in release mode. Documentation is also built
for the cache, and `runner --doc` will open that documentation in the browser. (It's
always nice to have local docs, especially in bandwidth-starved situations.)

It would be good to provide such an experience for the dynamic-link case, since
it is faster. There is in fact a dynamic cache as well but support for linking
against external crates dynamically is very basic. It works fine for crates that
don't have any external depdendencies, e.g. this creates a `libjson.so` in the
dynamic cache:

```
$ runner -C json
```
And then you can run the `json.rs` example without `-s`.

The `--compile` action takes three kinds of argument:

  - a crate name that is already loaded and known to Cargo
  - a Cargo directory
  - a Rust source file - the crate name is the file name without extension.

But anything more complicated is hard;  dynamic linking is not a priority for
Rust tooling at the moment, and does not support it well enough without terrible
hacking and use of unstable features.

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

With long crate names like this, it's useful to define _aliases_:

```
$ runner --alias es=easy_shortcuts
$ runner -xes -e 'es::argn_err(1,"gimme an arg!")'
...
```
By default, `runner -e` does a dynamic link, and there are known limitations.
By also using `--static`, you can evaluate expressions against crates
that can only be compiled as static libraries. So, assuming that we have
`time` in the static cache (`runner --add time` will do that for you):

```
$ runner -s -xtime -e "time::now()"
Tm { tm_sec: 34, tm_min: 4, tm_hour: 9, tm_mday: 28, tm_mon: 6, tm_year: 117,
tm_wday: 5, tm_yday: 208, tm_isdst: 0, tm_utcoff: 7200, tm_nsec: 302755857 }
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

