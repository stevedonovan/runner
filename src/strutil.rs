// miscelaneous string utilities
pub fn substitute<P,S>(text: &str, start: &str, until: P, subst: S) -> String
where P: Fn(char)->bool, S: Fn(&str)->String {
    let mut s = text;
    let mut out = String::new();
    while let Some(pos) = s.find(start) {
        out += &s[0..pos];
        s = &s[pos+1..];
        let end = s.find(|c:char| ! &until(c)).unwrap_or(s.len());
        let name = &s[0..end];
        out += &subst(name);
        s = &s[end..];
    }
    out += s;
    out
}
