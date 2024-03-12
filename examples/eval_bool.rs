{
  let a = true;
  let b = false;

  for a in [true, false] {
      for b in [true, false] {
            // Expression 1: !(!a && !b) using De Morgan's Law
            let expr1 = !(!a && !b);
            let expr2 = a || b;

            println!("a={a}, b={b}, !(!a && !b) = {expr1}, a || b = {expr2}"); // Output: !(!a && !b) = false
      }
    }
}
