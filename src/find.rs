/// Non-overlapping byte ranges of `needle` in `hay`. Empty needle matches nothing.
pub fn find_all(hay: &str, needle: &str, case: bool) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let (h, n) = if case { (hay.to_owned(), needle.to_owned()) } else { (fold(hay), fold(needle)) };
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = h[pos..].find(&n) {
        let s = pos + i;
        out.push((s, s + n.len()));
        pos = s + n.len();
    }
    out
}

/// Lowercase that never changes any char's byte length, so offsets map back to the original.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) if x.len_utf8() == c.len_utf8() => x,
                _ => c.to_ascii_lowercase(),
            }
        })
        .collect()
}

pub fn replace_one(text: &mut String, r: (usize, usize), with: &str) {
    text.replace_range(r.0..r.1, with);
}

pub fn replace_all(text: &str, needle: &str, with: &str, case: bool) -> (String, usize) {
    let matches = find_all(text, needle, case);
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for &(s, e) in &matches {
        out.push_str(&text[last..s]);
        out.push_str(with);
        last = e;
    }
    out.push_str(&text[last..]);
    (out, matches.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_sensitivity() {
        assert_eq!(find_all("aAa", "a", true), vec![(0, 1), (2, 3)]);
        assert_eq!(find_all("aAa", "a", false), vec![(0, 1), (1, 2), (2, 3)]);
    }

    #[test]
    fn overlap_is_not_double_counted() {
        assert_eq!(find_all("aaa", "aa", true), vec![(0, 2)]);
    }

    #[test]
    fn unicode_ranges_are_char_boundaries() {
        let h = "h\u{e9}llo \u{e9}\u{e9}";
        let m = find_all(h, "\u{e9}", true);
        assert_eq!(m.len(), 3);
        for (s, e) in m {
            assert_eq!(&h[s..e], "\u{e9}");
        }
        let m = find_all("\u{c9}COLE", "\u{e9}cole", false);
        assert_eq!(m.len(), 1);
        assert_eq!(&"\u{c9}COLE"[m[0].0..m[0].1], "\u{c9}COLE");
    }

    #[test]
    fn empty_needle_and_empty_hay() {
        assert!(find_all("abc", "", true).is_empty());
        assert!(find_all("", "a", true).is_empty());
    }

    #[test]
    fn needle_at_end() {
        assert_eq!(find_all("xxab", "ab", true), vec![(2, 4)]);
    }

    #[test]
    fn replace_all_counts_and_terminates() {
        assert_eq!(replace_all("a-a-a", "a", "aa", true), ("aa-aa-aa".to_string(), 3));
        assert_eq!(replace_all("abc", "", "x", true), ("abc".to_string(), 0));
        assert_eq!(replace_all("Foo foo", "foo", "bar", false), ("bar bar".to_string(), 2));
    }

    #[test]
    fn replace_one_replaces_range() {
        let mut s = "hello world".to_string();
        replace_one(&mut s, (6, 11), "there");
        assert_eq!(s, "hello there");
    }
}
