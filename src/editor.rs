//! Plain-text editor widget: line-number gutter + syntax-highlighted `TextEdit`.

use crate::format::Format;
use crate::highlight::{self, Span};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Id, ScrollArea, TextBuffer, TextEdit, Ui, Vec2,
    text::CCursor, text::CCursorRange,
};
use std::hash::{Hash, Hasher};

const EDITOR_ID: &str = "turbo-text-editor";

#[derive(Default)]
pub struct HlCache {
    key: u64,
    fmt: Option<Format>,
    spans: Vec<Span>,
}

impl HlCache {
    fn spans(&mut self, fmt: Format, text: &str) -> &[Span] {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        let key = h.finish();
        if self.fmt != Some(fmt) || self.key != key {
            self.spans = highlight::tokenize(fmt, text);
            self.key = key;
            self.fmt = Some(fmt);
        }
        &self.spans
    }
}

pub struct Opts<'a> {
    pub fmt: Format,
    pub wrap: bool,
    pub line_numbers: bool,
    pub font_size: f32,
    pub read_only: bool,
    pub highlight: bool,
    pub font_proportional: bool,
    /// 0 = Tab inserts a real tab; otherwise Tab inserts spaces up to the next tab stop.
    pub tab_spaces: usize,
    pub marks: &'a [(usize, usize)],
    pub current: Option<usize>,
    /// Select this byte range and scroll it into view (once).
    pub select: Option<(usize, usize)>,
}

/// Returns true if the text was edited.
pub fn show(ui: &mut Ui, text: &mut String, o: &Opts, cache: &mut HlCache) -> bool {
    let font = if o.font_proportional {
        FontId::proportional(o.font_size)
    } else {
        FontId::monospace(o.font_size)
    };
    let dark = ui.visuals().dark_mode;
    let id = Id::new(EDITOR_ID);
    let avail = ui.available_size();
    let lines = text.bytes().filter(|&b| b == b'\n').count() + 1;
    let digits = lines.to_string().len().max(2);
    let digit_w = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter_w = if o.line_numbers {
        digits as f32 * digit_w + 18.0
    } else {
        0.0
    };

    let mut changed = false;
    if o.tab_spaces > 0
        && !o.read_only
        && ui.memory(|m| m.has_focus(id))
        && ui.input_mut(consume_plain_tab)
    {
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        if let Some(r) = state.cursor.char_range() {
            let at = indent_insert(text, (r.primary.index.0, r.secondary.index.0), o.tab_spaces);
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(CCursor::new(at))));
            state.store(ui.ctx(), id);
            changed = true;
        }
    }
    let mut scroll_to_char = None;
    if let Some((a, b)) = o.select.filter(|(a, b)| {
        a <= b && *b <= text.len() && text.is_char_boundary(*a) && text.is_char_boundary(*b)
    }) {
        let (ca, cb) = (text[..a].chars().count(), text[..b].chars().count());
        let mut state = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(ca), CCursor::new(cb))));
        state.store(ui.ctx(), id);
        scroll_to_char = Some(ca);
    }

    let area = if o.wrap {
        ScrollArea::vertical()
    } else {
        ScrollArea::both()
    };
    area.auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(gutter_w);

            let mut layouter = |ui: &Ui, buf: &dyn TextBuffer, wrap_width: f32| {
                let t = buf.as_str();
                let spans: &[Span] = if o.highlight {
                    cache.spans(o.fmt, t)
                } else {
                    &[]
                };
                let mut job =
                    highlight::layout_job(t, spans, o.marks, o.current, font.clone(), dark);
                job.wrap.max_width = if o.wrap { wrap_width } else { f32::INFINITY };
                job.keep_trailing_whitespace = true;
                ui.fonts_mut(|f| f.layout_job(job))
            };

            let mut ro: &str = text.as_str();
            let buf: &mut dyn TextBuffer = if o.read_only { &mut ro } else { text };
            let out = TextEdit::multiline(buf)
                .id(id)
                .code_editor()
                .font(font.clone())
                .frame(egui::Frame::NONE)
                .layouter(&mut layouter)
                .desired_width((avail.x - gutter_w).max(50.0))
                .min_size(Vec2::new(0.0, avail.y))
                .show(ui);
            changed |= out.response.response.changed();

            if o.line_numbers {
                paint_gutter(ui, &out.galley, out.galley_pos, gutter_w, &font);
            }
            // Not `out.cursor_range`: egui only reports it while the TextEdit has focus, and
            // after "find next" the focus is in the find box.
            if let Some(c) = scroll_to_char {
                let r = out
                    .galley
                    .pos_from_cursor(CCursor::new(c))
                    .translate(out.galley_pos.to_vec2());
                ui.scroll_to_rect(r, Some(Align::Center));
            }
        });
    });
    changed
}

/// Consumes a Tab press unless Shift is held: `consume_key` ignores an extra Shift, which would turn
/// Shift+Tab into an indent instead of egui's built-in de-indent.
fn consume_plain_tab(i: &mut egui::InputState) -> bool {
    !i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
}

/// Replaces the selection (char indices, any order, clamped) with a tab: spaces up to the next
/// multiple of `spaces` columns (`spaces == 0` inserts a real tab). Returns the new cursor char index.
pub fn indent_insert(text: &mut String, sel: (usize, usize), spaces: usize) -> usize {
    let total = text.chars().count();
    let (a, b) = (sel.0.min(sel.1).min(total), sel.0.max(sel.1).min(total));
    let byte = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(i, _)| i);
    let (ba, bb) = (byte(a), byte(b));
    let col = text[..ba].chars().rev().take_while(|&c| c != '\n').count();
    let insert = if spaces == 0 {
        "\t".to_string()
    } else {
        " ".repeat(spaces - col % spaces)
    };
    text.replace_range(ba..bb, &insert);
    a + insert.chars().count()
}

fn paint_gutter(ui: &Ui, galley: &egui::Galley, origin: egui::Pos2, gutter_w: f32, font: &FontId) {
    let clip = ui.clip_rect();
    let color = if ui.visuals().dark_mode {
        Color32::from_gray(110)
    } else {
        Color32::from_gray(150)
    };
    let x = origin.x - 10.0;
    let _ = gutter_w;
    let (mut n, mut start) = (1usize, true);
    for row in &galley.rows {
        let y = origin.y + row.pos.y;
        if start && y + row.size.y >= clip.min.y && y <= clip.max.y {
            ui.painter().text(
                egui::pos2(x, y),
                Align2::RIGHT_TOP,
                n.to_string(),
                font.clone(),
                color,
            );
        }
        start = row.ends_with_newline;
        if start {
            n += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{consume_plain_tab, indent_insert};
    use eframe::egui::{self, Event, Key, Modifiers, RawInput};

    fn tab_event(modifiers: Modifiers) -> RawInput {
        RawInput {
            events: vec![
                Event::ModifiersChanged(modifiers),
                Event::Key {
                    key: Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            ..Default::default()
        }
    }

    fn consumed(modifiers: Modifiers) -> (bool, bool) {
        let ctx = egui::Context::default();
        let mut result = (false, false);
        let mut out = ctx.run_ui(tab_event(modifiers), |ui| {
            let ctx = ui.ctx();
            result.0 = ctx.input_mut(consume_plain_tab);
            // is the Tab key event still available to egui's own handling?
            result.1 = ctx.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, Event::Key { key: Key::Tab, .. }))
            });
        });
        out.textures_delta.clear();
        result
    }

    #[test]
    fn plain_tab_is_consumed_but_shift_tab_is_left_for_egui_to_dedent() {
        assert_eq!(consumed(Modifiers::NONE), (true, false));
        assert_eq!(
            consumed(Modifiers::SHIFT),
            (false, true),
            "Shift+Tab must stay unconsumed"
        );
    }

    fn run(text: &str, sel: (usize, usize), spaces: usize) -> (String, usize) {
        let mut t = text.to_string();
        let cur = indent_insert(&mut t, sel, spaces);
        (t, cur)
    }

    #[test]
    fn inserts_up_to_next_tab_stop() {
        assert_eq!(run("", (0, 0), 4), ("    ".into(), 4));
        assert_eq!(run("ab", (2, 2), 4), ("ab  ".into(), 4));
        assert_eq!(run("abcd", (4, 4), 4), ("abcd    ".into(), 8));
        assert_eq!(run("abc", (1, 1), 2), ("a bc".into(), 2));
    }

    #[test]
    fn column_counts_chars_not_bytes_and_resets_after_newline() {
        assert_eq!(run("\u{e9}\u{e9}", (2, 2), 4), ("\u{e9}\u{e9}  ".into(), 4));
        assert_eq!(run("a\nbc", (4, 4), 4), ("a\nbc  ".into(), 6));
        assert_eq!(run("a\n", (2, 2), 4), ("a\n    ".into(), 6));
    }

    #[test]
    fn replaces_selection_and_accepts_reversed_or_oversized_ranges() {
        assert_eq!(run("hello world", (0, 5), 4), ("     world".into(), 4));
        assert_eq!(run("hello world", (5, 0), 4), ("     world".into(), 4));
        assert_eq!(run("ab", (1, 99), 4), ("a   ".into(), 4));
        assert_eq!(run("ab", (50, 50), 4), ("ab  ".into(), 4));
    }

    #[test]
    fn zero_spaces_inserts_a_real_tab() {
        assert_eq!(run("ab", (1, 1), 0), ("a\tb".into(), 2));
    }
}
