// miscelaneous string utilities

// substitute patterns in a string, using a mapping function.
// You also provide a `start` for the pattern, and a predicate `until`
// for stopping it.
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

// returns the string slice following the target, if any
pub fn after<'a>(s: &'a str, target: &str) -> Option<&'a str> {
    if let Some(idx) = s.find(target) {
        Some(&s[(idx+target.len())..])
    } else {
        None
    }
}

// like after, but subsequently finds a word following...
pub fn word_after(txt: &str, target: &str) -> Option<String> {
    if let Some(txt) = after(txt,target) {
        // maybe skip some space, and end with whitespace or semicolon
        let start = txt.find(|c:char| c.is_alphanumeric()).unwrap();
        let end = txt.find(|c:char| c == ';' || c.is_whitespace()).unwrap();
        Some((&txt[start..end]).to_string())
    } else {
        None
    }
}

// next two items from an iterator, assuming that it has at least two items...
pub fn next_2<T, I: Iterator<Item=T>> (mut iter: I) -> (T,T) {
    (iter.next().unwrap(), iter.next().unwrap())
}

