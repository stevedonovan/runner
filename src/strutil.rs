// miscelaneous string utilities
use std::error::Error;

pub fn substitute<P,S,E>(text: &str, start: &str, until: P, subst: S) -> Result<String,Box<Error>>
where P: Fn(char)->bool, S: Fn(&str)->Result<String,E>, E: Error + 'static {
    let mut s = text;
    let mut out = String::new();
    while let Some(pos) = s.find(start) {
        out += &s[0..pos];
        s = &s[pos+1..];
        let end = s.find(|c:char| ! &until(c)).unwrap_or(s.len());
        let name = &s[0..end];
        out += &subst(name)?;
        s = &s[end..];
    }
    out += s;
    Ok(out)
}
