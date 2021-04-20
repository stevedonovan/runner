    // miscelaneous string utilities

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

// split into two at a delimiter
pub fn split(txt: &str, delim: char) -> (&str,&str) {
    if let Some(idx) = txt.find(delim) {
        (&txt[0..idx], &txt[idx+1..])
    } else {
        (txt,"")
    }
}

