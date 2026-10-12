use crate::format::Format;
use crate::theme::Palette;
use eframe::egui::{Color32, FontId, TextFormat, text::LayoutJob};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Key,
    String,
    Number,
    Keyword,
    Comment,
    Heading,
    Emph,
    Code,
    Link,
    Marker,
    Section,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

/// Byte-offset spans, sorted and non-overlapping, for the non-plain parts of `text`.
pub fn tokenize(fmt: Format, text: &str) -> Vec<Span> {
    let mut out = Vec::new();
    match fmt {
        Format::Json => json(text, &mut out),
        Format::Env => key_value(text, false, &mut out),
        Format::Config => key_value(text, true, &mut out),
        Format::Markdown => markdown(text, &mut out),
        Format::Plain => {}
    }
    out
}

fn push(out: &mut Vec<Span>, start: usize, end: usize, kind: Kind) {
    if start < end {
        out.push(Span { start, end, kind });
    }
}

/// (byte offset, line) pairs, split on '\n'.
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut off = 0;
    text.split('\n').map(move |l| {
        let o = off;
        off += l.len() + 1;
        (o, l)
    })
}

fn json(text: &str, out: &mut Vec<Span>) {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'"' && b[j] != b'\n' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = if j < b.len() && b[j] == b'"' {
                    j + 1
                } else {
                    j.min(b.len())
                };
                let mut k = end;
                while k < b.len() && (b[k] == b' ' || b[k] == b'\t') {
                    k += 1;
                }
                let kind = if k < b.len() && b[k] == b':' {
                    Kind::Key
                } else {
                    Kind::String
                };
                push(out, i, end, kind);
                i = end.max(i + 1);
            }
            b'-' | b'0'..=b'9' => {
                let mut j = i + 1;
                while j < b.len() && matches!(b[j], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
                {
                    j += 1;
                }
                push(out, i, j, Kind::Number);
                i = j;
            }
            c if c.is_ascii_alphabetic() => {
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_alphanumeric() {
                    j += 1;
                }
                if matches!(&text[i..j], "true" | "false" | "null") {
                    push(out, i, j, Kind::Keyword);
                }
                i = j;
            }
            _ => i += 1,
        }
    }
}

/// .env / ini / toml / yaml-ish: `key = value`, `# comment`, optional `[section]`.
fn key_value(text: &str, config: bool, out: &mut Vec<Span>) {
    for (off, line) in lines(text) {
        let ws = line.len() - line.trim_start().len();
        let rest = &line[ws..];
        if rest.is_empty() {
            continue;
        }
        if rest.starts_with('#') || (config && rest.starts_with(';')) {
            push(out, off + ws, off + line.trim_end().len(), Kind::Comment);
            continue;
        }
        if config && rest.starts_with('[') {
            push(out, off + ws, off + line.trim_end().len(), Kind::Section);
            continue;
        }
        let mut p = ws;
        if !config && rest.starts_with("export ") {
            push(out, off + p, off + p + 6, Kind::Keyword);
            p += 7;
            p += line[p..].len() - line[p..].trim_start().len();
        }
        let sep = line[p..].find(|c| c == '=' || (config && c == ':'));
        let Some(sep) = sep.map(|s| s + p) else {
            continue;
        };
        let key_end = p + line[p..sep].trim_end().len();
        push(out, off + p, off + key_end, Kind::Key);
        let mut v = sep + 1;
        v += line[v..].len() - line[v..].trim_start().len();
        value(line, off, v, out);
    }
}

fn value(line: &str, off: usize, v: usize, out: &mut Vec<Span>) {
    let rest = &line[v..];
    let Some(first) = rest.chars().next() else {
        return;
    };
    let tail_from = if first == '"' || first == '\'' {
        let b = rest.as_bytes();
        let mut j = 1;
        while j < b.len() && b[j] != first as u8 {
            j += if first == '"' && b[j] == b'\\' { 2 } else { 1 };
        }
        let end = (j + 1).min(b.len());
        push(out, off + v, off + v + end, Kind::String);
        v + end
    } else {
        let hash = rest
            .char_indices()
            .find(|&(i, c)| c == '#' && i > 0 && rest[..i].ends_with([' ', '\t']))
            .map(|(i, _)| i);
        let body = rest[..hash.unwrap_or(rest.len())].trim_end();
        let kind = if body.parse::<f64>().is_ok() {
            Kind::Number
        } else if body.eq_ignore_ascii_case("true") || body.eq_ignore_ascii_case("false") {
            Kind::Keyword
        } else {
            Kind::String
        };
        push(out, off + v, off + v + body.len(), kind);
        v + body.len()
    };
    let t = line[tail_from..].trim_start();
    if t.starts_with('#') {
        let s = line.len() - t.len();
        push(out, off + s, off + line.trim_end().len(), Kind::Comment);
    }
}

fn markdown(text: &str, out: &mut Vec<Span>) {
    let mut in_fence = false;
    for (off, line) in lines(text) {
        let ws = line.len() - line.trim_start().len();
        let rest = &line[ws..];
        let end = off + line.trim_end().len();
        if rest.starts_with("```") || rest.starts_with("~~~") {
            in_fence = !in_fence;
            push(out, off + ws, end, Kind::Code);
            continue;
        }
        if in_fence {
            push(out, off, end, Kind::Code);
            continue;
        }
        let hashes = rest.bytes().take_while(|&c| c == b'#').count();
        if (1..=6).contains(&hashes) && (rest.len() == hashes || rest.as_bytes()[hashes] == b' ') {
            push(out, off + ws, end, Kind::Heading);
            continue;
        }
        let mut start = ws;
        let b = rest.as_bytes();
        if b.len() > 1 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ' {
            push(out, off + ws, off + ws + 1, Kind::Marker);
            start = ws + 1;
        } else {
            let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && b.len() > digits + 1 && b[digits] == b'.' && b[digits + 1] == b' ' {
                push(out, off + ws, off + ws + digits + 1, Kind::Marker);
                start = ws + digits + 1;
            }
        }
        inline(line, off, start, out);
    }
}

fn inline(line: &str, off: usize, start: usize, out: &mut Vec<Span>) {
    let b = line.as_bytes();
    let mut i = start;
    while i < b.len() {
        let hit = match b[i] {
            b'`' => line[i + 1..].find('`').map(|j| (i + 1 + j + 1, Kind::Code)),
            b'*' if line[i..].starts_with("**") => line[i + 2..]
                .find("**")
                .filter(|&j| j > 0)
                .map(|j| (i + 2 + j + 2, Kind::Emph)),
            b'*' if b.get(i + 1).is_some_and(|c| *c != b' ') => {
                line[i + 1..].find('*').map(|j| (i + 1 + j + 1, Kind::Emph))
            }
            b'[' => line[i + 1..].find(']').and_then(|j| {
                let close = i + 1 + j;
                if b.get(close + 1) == Some(&b'(') {
                    line[close + 2..]
                        .find(')')
                        .map(|k| (close + 2 + k + 1, Kind::Link))
                } else {
                    None
                }
            }),
            _ => None,
        };
        match hit {
            Some((end, kind)) => {
                push(out, off + i, off + end, kind);
                i = end;
            }
            None => i += 1,
        }
    }
}

/// Builds the colored text. `marks` are sorted, non-overlapping find matches (byte ranges);
/// `current` indexes the match drawn in the stronger highlight.
pub fn layout_job(
    text: &str,
    spans: &[Span],
    marks: &[(usize, usize)],
    current: Option<usize>,
    font: FontId,
    pal: &Palette,
    line_height: Option<f32>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = pal.fg;
    let mut cuts = vec![0, text.len()];
    cuts.extend(spans.iter().flat_map(|s| [s.start, s.end]));
    cuts.extend(marks.iter().flat_map(|m| [m.0, m.1]));
    cuts.retain(|&c| c <= text.len() && text.is_char_boundary(c));
    cuts.sort_unstable();
    cuts.dedup();
    let (mut si, mut mi) = (0, 0);
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        while si < spans.len() && spans[si].end <= a {
            si += 1;
        }
        while mi < marks.len() && marks[mi].1 <= a {
            mi += 1;
        }
        let fg = spans
            .get(si)
            .filter(|s| s.start <= a && b <= s.end)
            .map_or(plain, |s| pal.kind(s.kind));
        let bg = match marks.get(mi) {
            Some(m) if m.0 <= a && b <= m.1 => {
                if current == Some(mi) {
                    Color32::from_rgb(230, 140, 40)
                } else {
                    Color32::from_rgba_unmultiplied(230, 200, 40, 110)
                }
            }
            _ => Color32::TRANSPARENT,
        };
        let fmt = TextFormat {
            font_id: font.clone(),
            color: fg,
            background: bg,
            line_height,
            ..Default::default()
        };
        job.append(&text[a..b], 0.0, fmt);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::resolve;

    fn toks(fmt: Format, text: &str) -> Vec<(Kind, &str)> {
        tokenize(fmt, text)
            .iter()
            .map(|s| (s.kind, &text[s.start..s.end]))
            .collect()
    }

    fn assert_valid(fmt: Format, text: &str) {
        let mut prev = 0;
        for s in tokenize(fmt, text) {
            assert!(s.start >= prev && s.start < s.end && s.end <= text.len());
            assert!(text.is_char_boundary(s.start) && text.is_char_boundary(s.end));
            prev = s.end;
        }
    }

    #[test]
    fn json_basic() {
        let t = toks(Format::Json, r#"{"a": 1, "b": true, "c": null, "d":"x"}"#);
        assert_eq!(
            t,
            vec![
                (Kind::Key, "\"a\""),
                (Kind::Number, "1"),
                (Kind::Key, "\"b\""),
                (Kind::Keyword, "true"),
                (Kind::Key, "\"c\""),
                (Kind::Keyword, "null"),
                (Kind::Key, "\"d\""),
                (Kind::String, "\"x\""),
            ]
        );
    }

    #[test]
    fn json_escaped_quote_and_numbers() {
        let t = toks(Format::Json, r#"["a\"b", -1.5e+3]"#);
        assert_eq!(
            t,
            vec![(Kind::String, r#""a\"b""#), (Kind::Number, "-1.5e+3")]
        );
    }

    #[test]
    fn json_unterminated_and_multibyte() {
        assert_valid(Format::Json, "{\"abc");
        assert_valid(Format::Json, "{\"é\": 1, \"日本\": \"語\"}");
        let t = toks(Format::Json, "{\"é\": 1}");
        assert_eq!(t[0], (Kind::Key, "\"é\""));
    }

    #[test]
    fn env_lines() {
        let t = toks(Format::Env, "# hi\nKEY=val # c\nexport K=\"v\"\n");
        assert_eq!(
            t,
            vec![
                (Kind::Comment, "# hi"),
                (Kind::Key, "KEY"),
                (Kind::String, "val"),
                (Kind::Comment, "# c"),
                (Kind::Keyword, "export"),
                (Kind::Key, "K"),
                (Kind::String, "\"v\""),
            ]
        );
    }

    #[test]
    fn env_hash_inside_value_is_not_comment() {
        let t = toks(Format::Env, "URL=http://x/#frag\n");
        assert_eq!(
            t,
            vec![(Kind::Key, "URL"), (Kind::String, "http://x/#frag")]
        );
    }

    #[test]
    fn ini_and_toml() {
        let t = toks(Format::Config, "[sec]\nk = v\n; c\n# d\nn = 5\nb = true\n");
        assert_eq!(
            t,
            vec![
                (Kind::Section, "[sec]"),
                (Kind::Key, "k"),
                (Kind::String, "v"),
                (Kind::Comment, "; c"),
                (Kind::Comment, "# d"),
                (Kind::Key, "n"),
                (Kind::Number, "5"),
                (Kind::Key, "b"),
                (Kind::Keyword, "true"),
            ]
        );
    }

    #[test]
    fn markdown_blocks_and_inline() {
        let t = toks(Format::Markdown, "# H\n- item **b** and *i* `c` [t](u)\n");
        assert_eq!(
            t,
            vec![
                (Kind::Heading, "# H"),
                (Kind::Marker, "-"),
                (Kind::Emph, "**b**"),
                (Kind::Emph, "*i*"),
                (Kind::Code, "`c`"),
                (Kind::Link, "[t](u)"),
            ]
        );
    }

    #[test]
    fn markdown_fence_and_unterminated() {
        let t = toks(Format::Markdown, "```rust\nlet a = *b*;\n```\nafter\n");
        assert_eq!(
            t,
            vec![
                (Kind::Code, "```rust"),
                (Kind::Code, "let a = *b*;"),
                (Kind::Code, "```"),
            ]
        );
        assert_valid(Format::Markdown, "```\nnever closed\n**oops");
        assert_valid(Format::Markdown, "[a](b\n`x\n*é*");
    }

    #[test]
    fn nasty_inputs_never_panic_and_stay_valid() {
        let inputs = [
            "",
            "\n\n",
            "\"",
            "\"\\",
            "\"\\\u{e9}",
            "[",
            "[](",
            "[a](",
            "`",
            "**",
            "*",
            "* ",
            "export ",
            "export",
            "=",
            "= =",
            "a=\"",
            "a='",
            "a=\"\\",
            "#",
            "```",
            "~~~\n",
            "1.",
            "1. ",
            "# ",
            "#######",
            "\u{65e5}\u{672c}=\u{8a9e} # \u{e9}",
            "k: \"\u{e9}",
            "-",
            "-e",
            "tru",
            "\r\n",
            "a\r\n=b\r\n",
        ];
        for f in [
            Format::Json,
            Format::Env,
            Format::Config,
            Format::Markdown,
            Format::Plain,
        ] {
            for t in inputs {
                assert_valid(f, t);
            }
        }
    }

    #[test]
    fn layout_job_marks_matches_with_background() {
        let text = "ab cd ab";
        let spans = tokenize(Format::Plain, text);
        let marks = [(0, 2), (6, 8)];
        let job = layout_job(
            text,
            &spans,
            &marks,
            Some(1),
            FontId::monospace(14.0),
            resolve("dark", true),
            None,
        );
        assert_eq!(job.text, text);
        let bg = |r: std::ops::Range<usize>| {
            job.sections
                .iter()
                .find(|s| s.byte_range.start.0 == r.start && s.byte_range.end.0 == r.end)
                .map(|s| s.format.background)
        };
        assert_ne!(bg(0..2), Some(Color32::TRANSPARENT));
        assert_ne!(bg(6..8), Some(Color32::TRANSPARENT));
        assert_ne!(bg(0..2), bg(6..8), "current match differs from others");
        assert_eq!(bg(2..6), Some(Color32::TRANSPARENT));
    }

    #[test]
    fn layout_job_handles_marks_inside_spans() {
        let text = "{\"key\": 1}";
        let spans = tokenize(Format::Json, text);
        let job = layout_job(
            text,
            &spans,
            &[(2, 4)],
            None,
            FontId::monospace(14.0),
            resolve("light", false),
            None,
        );
        assert_eq!(job.text, text);
    }

    #[test]
    fn layout_job_applies_line_height_to_every_section() {
        let text = "k = 1 # c";
        let spans = tokenize(Format::Config, text);
        let pal = resolve("nord", true);
        let font = FontId::monospace(14.0);
        let tall = layout_job(text, &spans, &[(0, 1)], None, font.clone(), pal, Some(30.0));
        assert!(tall.sections.len() > 2);
        assert!(
            tall.sections
                .iter()
                .all(|s| s.format.line_height == Some(30.0))
        );
        let plain = layout_job(text, &spans, &[], None, font, pal, None);
        assert!(
            plain
                .sections
                .iter()
                .all(|s| s.format.line_height.is_none())
        );
    }

    #[test]
    fn layout_job_uses_the_palette_colours() {
        let text = "k = 1";
        let spans = tokenize(Format::Config, text);
        let font = FontId::monospace(14.0);
        let nord = layout_job(
            text,
            &spans,
            &[],
            None,
            font.clone(),
            resolve("nord", true),
            None,
        );
        let dracula = layout_job(
            text,
            &spans,
            &[],
            None,
            font,
            resolve("dracula", true),
            None,
        );
        assert_eq!(
            nord.sections[0].format.color,
            resolve("nord", true).kind(Kind::Key)
        );
        assert_ne!(
            nord.sections[0].format.color,
            dracula.sections[0].format.color
        );
    }

    #[test]
    fn plain_has_no_spans() {
        assert!(tokenize(Format::Plain, "anything # here").is_empty());
    }
}
