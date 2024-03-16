#!/usr/bin/env runner
//: --static

use itertools::Itertools;

let fib = |n: usize| -> usize {
    itertools::iterate((0, 1), |&(a, b)| (b, a + b))
        .take(n + 1)
        .last()
        .unwrap()
        .0
};
println!("Type lines of text at the prompt and hit Ctrl-D when done");
let mut buffer = String::new();
io::stdin().lock().read_to_string(&mut buffer)?;
let n: usize = buffer.trim_end()
    .parse()
    .expect("Can't parse input into a positive integer");
let f = fib(n);
println!("Fibonacci number {n} is {f}");
