# Running Little Rust Snippets

## Leaving the Comfort (and Restrictions) of Cargo

Cargo is a good, reliable way to build programs and libraries in Rust with versioned dependencies.
Those of us who have worked with the Wild West practices of C++ development find this particularly soothing,
and it's one of the core strengths of the Rust ecosystem.

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
println!("Hello, World!");

$ runner print.rs
Hello, World!
```
This follows basically the same rules as the doc-test snippets you find in Rust
documentation, so `runner` allows you to copy those snippets into an editor
and directly run them (I bind 'run' for Rust projects to `runner ...` in
my favourite editor.)

You can use `?` in snippets instead of the ubiquitous and awful `unwrap`, since the boilerplate
encloses code in a function that returns `Result<(),Box<Error+Sync+Send>>` which is compatible with
any error return.

A special variable `args` is available containing any arguments passed to the program:

```
$ cat hello.rs
println!("hello {}",args[1]);
$ runner hello.rs dolly
hello dolly
```

You can even -- on Unix/Linux and Mac platforms -- add a 'shebang' line to invoke runner:

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
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_macros)]
use std::{fs,io,env};
use std::fs::File;
use std::io::prelude::*;
use std::path::{PathBuf,Path};
use std::collections::HashMap;
use std::time::Duration;
use std::thread;

macro_rules! debug {
    ($x:expr) => {
        println!("{} = {:?}",stringify!($x),$x);
    }
}
```

After first invocation of `runner`, this is found in `~/.cargo/.runner/prelude`;
you can edit it later with `runner --edit-prelude`.

`debug!` saves typing: `debug!(my_var)` is equivalent to `println!("my_var = {:?}",my_var)`.

As an experimental feature, `runner` will also do some massaging of `rustc` errors.
They are usually very good, but involve fully qualified type names.
It reduces `std::` references to something simpler.

This is a snippet which a Java programmer would find easy to write - declare that type explicitly,
and assume that the important verb is "set":

```
$ cat testm.rs
let mut map: HashMap<String,String> = HashMap::new();
map.set("hello","dolly");
$  runner testm.rs
error[E0599]: no method named `set` found for type `HashMap<String, String>` in the current scope
  --> /home/steve/.cargo/.runner/bin/testm.rs:24:9
   |
24 |     map.set("hello","dolly");
   |         ^^^
   |
   = help: did you mean `get`?
```

Since we are being very _informal_ with Rust here, it's appropriate that we don't wish the type spelled
out in full glory (as you can see by running with `-S`):
 `std::collections::HashMap<std::string::String, std::string::String>`.

## Adding External Crates

As you can see, `runner` is very much about playing with small code snippets. By
default it links the snippet _dynamically_ which is significantly faster.

The static option is much more convenient. You can easily create a static
cache with some common crates:

```
$ runner --add "time json regex"
```

A local crate may be specified by its pathname.
You can add as many crates as you like - the number of available dependencies doesn't
slow down the linker. Thereafter, you may refer to these crates in snippets. Note that
by default, `runner` uses 2021 edition since 0.6.0.

```rust
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
```

And then build statically and run (any extra arguments are passed to the program.)

```json
$ runner -s json.rs
{"code":200,"success":true,"payload":{"features":["awesome","easyAPI","lowLearningCurve"]}}
```

A convenient new feature is "argument lines" - if the first line of `json.rs` was

```
//: -s
```

then any `runner` arguments specified after "//:" will be merged in with the command-line arguments.
It is now possible to simply invoke using `runner json.rs`. It's better to keep any special build
instructions in the file itself, and it means that an editor run action bound to `runner FILE` can be
made to work in all cases.

`runner` provides various utilities for managing the static cache.
You can say `runner --edit` to edit the static cache `Cargo.toml`, and `runner --build` to
rebuild the cache afterwards. `runner --update` will update all the dependencies in the
cache, and `runner --update package` will update a _particular_ package - follow this
with `--build` as before.

 The cache is built for both debug and release mode,
so using `-sO` you can build snippets in release mode. Documentation is also built
for the cache, and `runner --doc` will open that documentation in the browser. (It's
always nice to have local docs, especially in bandwidth-starved situations.)

If you want docs for a specific crate `NAME`, then `runner --doc NAME` will work.
Remember that the Rust documentation generated has a fast offline searchable
index!

The `--crates` command also has an optional argument; without arguments it lists all
he crates known to `runner`, with their versions. With a name, it uses an exact match:

```
$ runner --crates yansi
yansi = "0.3.4"
```

You may provide a number of crate names here; if `--verbose` (`-v`) is specified
then the dependencies of these crates are also listed.

The `-c` flag only compiles the program or snippet, and copies it to `~/.cargo/bin`.
`-r` only runs the program, which must have previously been compiled, either
explicitly with `-c` or implicitly with default operation.

Plain Rust source files (which already have `fn main`) are of course supported, but you
will need the `--extern` (`-x`) flag to bring in any external crates from the static cache.

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
error[E0277]: the trait bound `{integer}: Mul<{float}>` is not satisfied
  --> temp/tmp.rs:20:22
   |
20 |     let res = 10 + 20*4.5;
   |                      ^ no implementation for `{integer} * {float}`
```

Likewise, you have to say `1.2f64.sin()` because `1.2` has ambiguous type.

(Note that the trait `std::ops::Mul` is presented in _simplified form_ by default)

`--expression` is very useful if you quickly want to find out how Rust
will evaluate an expression - we do a debug print for maximum flexibility.

```
$ runner -e 'PathBuf::from("bonzo.dog").extension()'
Some("dog")
```

(This works because we have a `use std::path::PathBuf` in the runner prelude.)

Now, this will not work on Windows since [quoting](https://stackoverflow.com/questions/7760545/escape-double-quotes-in-parameter)
is seriously baroque. So `runner` re-uses an old trick that some Windows versions of `AWK` used. We can
only use double-quotes for an argument that may contain spaces, but single-quotes within this will
be converted to double-quotes.

```
c:> runner -e "PathBuf::from('bonzo.dog').extension()"
Some("dog")
```

So, in these examples where you need to quote strings in the Rust expression,
remember that it works the other way in Windows.

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
$ echo "hello\nthere" | runner -n 'line.to_uppercase()'
"HELLO"
"THERE"
```
The `-x` flag (`--extern`) allows you to insert an `extern crate` into your
snippet. This is particularly useful for these one-line shortcuts. For
example, my `easy-shortcuts` crate has a couple of helper functions. Before
running the following examples, first `runner --add easy-shortcuts` to load it into the
static crate, and then `runner -C easy-shortcuts` to dynamically compile it.

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

With `-e`,`-n` or `-i`, you can specify. some initial code with `--prepend`:

```
$  runner -p 'let nums=0..5' -i 'nums.clone().zip(nums.skip(1))'
(0, 1)
(1, 2)
(2, 3)
(3, 4)
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

'-X' (`--wild`) is like `-x` except it brings all the crate's symbols into scope.
Not something you would overdo in regular code, but it makes for shorter
command lines - the last example becomes (note how short flags can be combined):

```
$ runner -sXtime -e "now()"
...
```
`-M` (`--macro`) is also like `-x` except it prepends the 'extern crate' with
`#[macro_use]`.  Consider the very cool [r4](https://docs.rs/r4) crate which
provides list comprehensions. First load in the static cache with `runner --add r4`,
and then we can say:

```
$ runner -s --macro r4 -i 'iterate![for x in 0..4; yield x]'
0
1
2
3
```

Small snippets like these are faster if the crates can be linked dynamically, so
after `runner -C r4` to build a shared library in the dynamic cache, you can run this
without the `-s`.

```
$ runner --macro r4 -i 'iterate![for x in 0..4; yield x]'
0
1
2
3
```

Here's a fancier dynamic example. We'll need to include the easy_shortcuts crate which we aliased to "es" above, and prepend an import statement for its
ToVec trait.
So in preparation run any of the following that you haven't already run:


```
$ runner -C r4
$ runner -C easy_shortcuts
$ runner --alias es=easy_shortcuts
```

Now we can run our dynamic snippet:

```
$ runner -xes -p 'use es::traits::ToVec' -Mr4 -e 'iterate![for i in 0..2; for j in 0..2; yield (i,j)].to_vec()'
[(0, 0), (0, 1), (1, 0), (1, 1)]
```

(At this point, the command-line is getting sufficiently complicated that you would
be better off with a little snippet that you can edit in a proper editor.)

If you can get away with dynamic linking, then `runner` can make it
easy to test a module interactively. In this way you get much of the
benefit of a fully interactive interpreter (a REPL):

```
$ cat universe.rs
pub fn answer() -> i32 {
    42
}
$ runner -C universe.rs
building crate 'universe' at universe.rs
$ runner -xuniverse -e "universe::answer()"
42
```
This provides a way to get to play with big predefined strings:

```
$ cat text.rs
pub const TEXT: &str = "possibly very long string";
$ runner -C text.rs
building crate 'text' at text.rs
$ runner -Xtext -e 'TEXT.find("long")'
Some(14)
```

## Compiling Rust Doc Examples

Consider the example for the [filetime](https://docs.rs/filetime) crate:

```rust
use std::fs;
use filetime::FileTime;

let metadata = fs::metadata("runner.rs").unwrap();

let mtime = FileTime::from_last_modification_time(&metadata);
println!("{}", mtime);

let atime = FileTime::from_last_access_time(&metadata);
assert!(mtime < atime);

// Inspect values that can be interpreted across platforms
println!("{}", mtime.seconds_relative_to_1970());
println!("{}", mtime.nanoseconds());

// Print the platform-specific value of seconds
println!("{}", mtime.seconds());
```

After `runner --add filetime`, this crate is in your static cache. And `runner --doc filetime`
will give you its local documentation.

However, it can't be compiled directly, for the reason that `use std::fs` is already in the runner prelude.

So we need to say:

```
$ runner -s --no-prelude filetime.rs
1506778536.945440909s
1506778536
945440909
1506778536
```

Or if you're in a hurry: `runner -sN filetime.rs`.

As always, can always put these arguments in a first comment like so "//: -sN".

## Dymamic Compilation of Crates

It would be good to provide such an experience for the dynamic-link case, since
it is faster. There is in fact a dynamic cache as well but support for linking
against external crates dynamically is very basic. It works fine for crates that
don't have any external dependencies, e.g. this creates a `libjson.so` in the
dynamic cache:

```
$ runner -C json
```
And then you can run the `json.rs` example without `-s`.

The `--compile` action takes three kinds of arguments:

  - a crate name that is already loaded and known to Cargo
  - a Cargo directory
  - a Rust source file - the crate name is the file name without extension.

Dynamic linking is not a priority for
Rust tooling at the moment. So we have to build more elaborate libraries without the
help of Cargo. (The following assumes that you have already brought in `regex` for a Cargo project,
so that the Cargo cache is populated, e.g. with `runner --add regex`)

TODO: Not working

```
runner -C memchr
runner -C aho-corasick
runner -C utf8-ranges
runner -C lazy_static
runner -xlazy_static -C thread_local
runner -C regex-syntax
runner -C regex

```
This script drives home how tremendously irritating life in Rust would be without Cargo.
We have to track the dependencies, ensure that the correct default features are enabled in the
compilation, and special-case crates which directly link to `libc`.

However, the results feel worthwhile. Compiling the first `regex` documented example:

```rust
use regex::Regex;
let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
assert!(re.is_match("2014-01-01"));
```
With a static build (`-s`) I get 0.56s on this machine, and 0.25s with dynamic linking.

There are limitations to dynamic linking currently - crates which are "no std"
(and don't provide a feature to turn this off) cannot be compiled.  Also, remember
that all invocations of `runner -C` end up with shared libraries placed in one
directory called the 'dynamic cache' - there can only be one crate called 'libs'
for example.
