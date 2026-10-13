use crate::highlight::Kind;
use crate::theme::Palette;
use eframe::egui::{self, Color32, RichText, ScrollArea, Ui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use serde_json::Value;

/// Returns (line, column, message) if `text` is not valid JSON. Empty text is invalid.
pub fn json_error(text: &str) -> Option<(usize, usize, String)> {
    serde_json::from_str::<Value>(text).err().map(|e| {
        let msg = e.to_string();
        // serde appends " at line N column M"; we report the position separately
        let msg = msg
            .rfind(" at line ")
            .map_or(msg.as_str(), |i| &msg[..i])
            .to_owned();
        (e.line(), e.column(), msg)
    })
}

/// Children shown per container before collapsing the rest into a "… n more" line.
const MAX_CHILDREN: usize = 1000;

pub fn json_tree(ui: &mut Ui, value: &Value, pal: &Palette) {
    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            node(ui, None, value, "root", 0, pal);
        });
}

fn leaf_color(v: &Value, pal: &Palette) -> Color32 {
    pal.kind(match v {
        Value::String(_) => Kind::String,
        Value::Number(_) => Kind::Number,
        _ => Kind::Keyword,
    })
}

fn node(ui: &mut Ui, key: Option<&str>, v: &Value, path: &str, depth: usize, pal: &Palette) {
    let key_color = pal.kind(Kind::Key);
    let prefix = key.map(|k| format!("{k}: ")).unwrap_or_default();
    let children: Vec<(String, &Value)> = match v {
        Value::Object(m) => m
            .iter()
            .take(MAX_CHILDREN)
            .map(|(k, v)| (k.clone(), v))
            .collect(),
        Value::Array(a) => a
            .iter()
            .take(MAX_CHILDREN)
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => {
            ui.horizontal(|ui| {
                if !prefix.is_empty() {
                    ui.label(RichText::new(&prefix).monospace().color(key_color));
                }
                let shown = match v {
                    Value::String(s) => format!("\"{s}\""),
                    other => other.to_string(),
                };
                ui.label(RichText::new(shown).monospace().color(leaf_color(v, pal)));
            });
            return;
        }
    };
    let (open, close, total) = match v {
        Value::Object(m) => ("{", "}", m.len()),
        Value::Array(a) => ("[", "]", a.len()),
        _ => unreachable!(),
    };
    let title = RichText::new(format!("{prefix}{open}{total}{close}"))
        .monospace()
        .color(key_color);
    egui::CollapsingHeader::new(title)
        .id_salt(path)
        .default_open(depth < 1)
        .show(ui, |ui| {
            for (k, child) in &children {
                let is_arr = matches!(v, Value::Array(_));
                let label = if is_arr { format!("[{k}]") } else { k.clone() };
                node(
                    ui,
                    Some(&label),
                    child,
                    &format!("{path}/{k}"),
                    depth + 1,
                    pal,
                );
            }
            if total > children.len() {
                ui.label(RichText::new(format!("… {} more", total - children.len())).italics());
            }
        });
}

/// Text sizes scale so the body matches `font_size` (the editor's size setting); headings and code follow.
/// `line_height` is read by the patched `egui_commonmark_backend` (see `vendor/`) from egui's temp
/// data; the body text styles use `family`, code blocks stay monospace.
pub fn markdown(
    ui: &mut Ui,
    cache: &mut CommonMarkCache,
    text: &str,
    family: &egui::FontFamily,
    font_size: f32,
    line_height: f32,
) {
    ui.data_mut(|d| d.insert_temp(egui::Id::new("turbo-text-line-height"), line_height));
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let body = ui.style().text_styles[&egui::TextStyle::Body].size;
            let scale = font_size / body;
            for (style, f) in ui.style_mut().text_styles.iter_mut() {
                f.size *= scale;
                if *style != egui::TextStyle::Monospace {
                    f.family = family.clone();
                }
            }
            ui.add_space(6.0);
            CommonMarkViewer::new().show(ui, cache, text);
        });
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
    fn message_does_not_repeat_position() {
        let (_, _, m) = json_error("{\"a\":}").unwrap();
        assert!(!m.contains(" at line "), "{m}");
        assert!(!m.is_empty());
    }

    #[test]
    fn empty_and_whitespace_are_invalid() {
        assert!(json_error("").is_some());
        assert!(json_error("  \n ").is_some());
    }
}
