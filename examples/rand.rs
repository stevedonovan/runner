//: --static
use rand::prelude::*;

println!("Random usize={}", rand::random::<usize>());

    let mut rng = rand::thread_rng();

    let mut nums: Vec<i32> = (1..=20).collect();
    nums.shuffle(&mut rng);
    println!("First 20 integers, randomly shuffled={nums:?}");
