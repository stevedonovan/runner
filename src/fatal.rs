use std::fmt::Display;
use std::process;

pub trait OrDie<T> {
    fn or_die(self, message: &str) -> T;
}

impl<T, E: Display> OrDie<T> for Result<T, E> {
    fn or_die(self, message: &str) -> T {
        self.unwrap_or_else(|err| quit(&format!("{}: {}", message, err)))
    }
}

impl<T> OrDie<T> for Option<T> {
    fn or_die(self, message: &str) -> T {
        self.unwrap_or_else(|| quit(message))
    }
}

pub trait OrThenDie<T> {
    fn or_then_die<F>(self, message: F) -> T
    where
        F: FnOnce(&dyn Display) -> String;
}

impl<T, E: Display> OrThenDie<T> for Result<T, E> {
    fn or_then_die<F>(self, message: F) -> T
    where
        F: FnOnce(&dyn Display) -> String,
    {
        self.unwrap_or_else(|err| quit(&message(&err)))
    }
}

impl<T> OrThenDie<T> for Option<T> {
    fn or_then_die<F>(self, message: F) -> T
    where
        F: FnOnce(&dyn Display) -> String,
    {
        self.unwrap_or_else(|| quit(&message(&"missing value")))
    }
}

pub fn quit(message: &str) -> ! {
    eprintln!("{}", message);
    process::exit(1);
}
