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
    pub marks: &'a [(usize, usize)],
    pub current: Option<usize>,
    /// Select this byte range and scroll it into view (once).
    pub select: Option<(usize, usize)>,
}

/// Returns true if the text was edited.
pub fn show(ui: &mut Ui, text: &mut String, o: &Opts, cache: &mut HlCache) -> bool {
    let font = FontId::monospace(o.font_size);
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

    let mut changed = false;
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
            changed = out.response.response.changed();

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
