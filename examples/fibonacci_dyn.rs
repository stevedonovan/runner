#!/usr/bin/env runner
//: --dynamic

let fib = |n: usize| -> usize {
    let mut a = 0;
    let mut b = 1;
    std::iter::repeat(()).take(n + 1).map(move |()| {
        std::mem::swap(&mut a, &mut b);
        let next = a + b;
        std::mem::replace(&mut b, next)
    }).last().expect("Argh!")
};

println!("Type lines of text at the prompt and hit Ctrl-D when done");
let mut buffer = String::new();
io::stdin().lock().read_to_string(&mut buffer)?;
let n: usize = buffer.trim_end()
    .parse()
    .expect("Can't parse input into a positive integer");
let f = fib(n);
println!("Fibonacci number {n} is {f}");
