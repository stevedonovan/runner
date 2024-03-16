use std::fmt;

pub struct PrettyError<T>(pub T);

impl<T> From<T> for PrettyError<T> {
    fn from(v: T) -> Self {
        Self(v)
    }
}

impl<T: fmt::Display> fmt::Debug for PrettyError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

// fn err1() -> Result<(), PrettyError<String>> {
//     Err(String::from("Line1\nLine2"))?;
//     Ok(())
// }

fn main() {
    let fmt = |e: Box<dyn std::error::Error>, f: &mut std::fmt::Formatter<'_>| -> fmt::Result {
        std::fmt::Display::fmt(&e, f)
    };

    let err1 = || -> Result<(), PrettyError<String>> {
        Err(String::from("Line1\nLine2"))?;
        Ok(())
    };

    let r = err1();
    println!("Result={r:?}");
}
