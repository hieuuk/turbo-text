/// Returns (line, column, message) if `text` is not valid JSON. Empty text is invalid.
pub fn json_error(text: &str) -> Option<(usize, usize, String)> {
    serde_json::from_str::<serde_json::Value>(text)
        .err()
        .map(|e| (e.line(), e.column(), e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_json_is_none() {
        assert!(json_error(r#"{"a": [1, 2, {"b": null}]}"#).is_none());
        assert!(json_error("  42  ").is_none());
    }

    #[test]
    fn reports_line_and_column() {
        let (l, c, m) = json_error("{\"a\":}").unwrap();
        assert_eq!(l, 1);
        assert!(c > 0 && !m.is_empty());
        let (l, _, _) = json_error("{\n  \"a\": 1,\n  \"b\": oops\n}").unwrap();
        assert_eq!(l, 3);
    }

    #[test]
    fn empty_and_whitespace_are_invalid() {
        assert!(json_error("").is_some());
        assert!(json_error("  \n ").is_some());
    }
}
