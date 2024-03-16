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

fn main() {
    let err1 = || -> Result<(), PrettyError<String>> {
        Err(String::from("Line1\nLine2"))?;
        Ok(())
    };

    let r = err1();
    println!("Result={r:?}");
}
