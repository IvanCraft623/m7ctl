//! Colour picker.

use crate::tui::*;

// ---- colour picker ----

pub(crate) const PAL_W: u16 = 60;
pub(crate) const PAL_ROWS: u16 = 8;
pub(crate) const BAR_X: u16 = 2;
pub(crate) const BAR_W: u16 = 50;
pub(crate) const PICKER_W: u16 = 66;
pub(crate) const PICKER_H: u16 = 23;
// lines inside the picker
pub(crate) const L_CHIPS: u16 = 0;
pub(crate) const L_PAL: u16 = 2; // 8 rows
pub(crate) const L_GREY: u16 = 10;
pub(crate) const L_BARS: u16 = 12; // R, G, B
pub(crate) const L_PRESETS: u16 = 16;
pub(crate) const L_BUTTONS: u16 = 18;
pub(crate) const OK_X: u16 = 40;
pub(crate) const CANCEL_X: u16 = 47;
pub(crate) const PRESETS: [[u8; 3]; 20] = [
    [255, 255, 255],
    [190, 190, 190],
    [110, 110, 110],
    [0, 0, 0],
    [255, 0, 0],
    [255, 90, 0],
    [255, 160, 0],
    [255, 230, 0],
    [160, 255, 0],
    [0, 255, 0],
    [0, 255, 150],
    [0, 255, 255],
    [0, 150, 255],
    [0, 0, 255],
    [110, 0, 255],
    [190, 0, 255],
    [255, 0, 255],
    [255, 0, 130],
    [150, 80, 20],
    [255, 150, 150],
];

#[derive(Clone, Debug)]
pub(crate) struct Picker {
    pub(crate) tab: usize,
    pub(crate) row: usize,
    pub(crate) colors: Vec<[u8; 3]>,
    pub(crate) idx: usize,
    pub(crate) chan: usize,
    pub(crate) max: usize,
    /// Digits typed into the selected channel (the value follows as they are typed).
    pub(crate) typed: String,
}

pub(crate) fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [((r + m) * 255.0).round() as u8, ((g + m) * 255.0).round() as u8, ((b + m) * 255.0).round() as u8]
}

pub(crate) fn palette_color(col: u16, line: u16) -> [u8; 3] {
    let l = 0.92 - 0.09 * line as f32;
    hsl_to_rgb(col as f32 * 360.0 / PAL_W as f32, 1.0, l)
}

pub(crate) fn hex_of(c: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub(crate) fn parse_color_list(text: &str) -> Vec<[u8; 3]> {
    let v: Vec<[u8; 3]> = text
        .split(',')
        .filter_map(|c| {
            let h = c.trim().trim_start_matches('#');
            if h.len() != 6 {
                return None;
            }
            let v = u32::from_str_radix(h, 16).ok()?;
            Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
        })
        .collect();
    if v.is_empty() {
        vec![[0, 0, 255]]
    } else {
        v
    }
}

pub(crate) fn rgb_color(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

pub(crate) fn contrast(c: [u8; 3]) -> Color {
    if c[0] as u32 * 299 + c[1] as u32 * 587 + c[2] as u32 * 114 > 128_000 {
        Color::Black
    } else {
        Color::White
    }
}

pub(crate) fn picker_rect(area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(PICKER_W) / 2,
        area.y + area.height.saturating_sub(PICKER_H) / 2,
        PICKER_W.min(area.width),
        PICKER_H.min(area.height),
    )
}

pub(crate) fn draw_picker(f: &mut Frame, app: &App, area: Rect) {
    let Some(p) = &app.modal.picker else { return };
    let r = picker_rect(area);
    f.render_widget(Clear, r);
    let cur = p.colors[p.idx];
    let mut lines: Vec<Line> = vec![];
    // chips
    let mut spans = vec![Span::raw("Colour ")];
    for (i, c) in p.colors.iter().enumerate() {
        let (l, rr) = if i == p.idx { ('[', ']') } else { (' ', ' ') };
        spans.push(Span::styled(format!("{l}{}{rr}", i + 1), Style::new().fg(contrast(*c)).bg(rgb_color(*c)).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled("[ + ]", Style::new().fg(Color::Green)));
    spans.push(Span::raw(" "));
    spans.push(Span::styled("[ - ]", Style::new().fg(Color::Red)));
    lines.push(Line::from(spans));
    lines.push(Line::from(Span::styled("Palette", Style::new().fg(Color::DarkGray))));
    for line in 0..PAL_ROWS {
        lines.push(Line::from((0..PAL_W).map(|c| Span::styled(" ", Style::new().bg(rgb_color(palette_color(c, line))))).collect::<Vec<_>>()));
    }
    lines.push(Line::from(
        (0..PAL_W)
            .map(|c| {
                let v = (c as u32 * 255 / (PAL_W as u32 - 1)) as u8;
                Span::styled(" ", Style::new().bg(Color::Rgb(v, v, v)))
            })
            .collect::<Vec<_>>(),
    ));
    lines.push(Line::from(Span::styled("Fine-tune", Style::new().fg(Color::DarkGray))));
    for (ch, name) in ["R", "G", "B"].iter().enumerate() {
        let sel = p.chan == ch;
        let mut spans = vec![Span::styled(format!("{name} "), if sel { Style::new().fg(Color::Black).bg(Color::Cyan) } else { Style::new() })];
        let v = cur[ch];
        for x in 0..BAR_W {
            let at = (x as u32 * 255 / (BAR_W as u32 - 1)) as u8;
            let mut c = cur;
            c[ch] = at;
            let filled = at <= v;
            spans.push(Span::styled(
                if filled { " " } else { "\u{2591}" },
                Style::new().bg(if filled { rgb_color(c) } else { Color::Rgb(40, 40, 40) }).fg(Color::DarkGray),
            ));
        }
        spans.push(Span::raw(format!(" {v:>3}{}", if sel && !p.typed.is_empty() { "_" } else { "" })));
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(Span::styled("Presets", Style::new().fg(Color::DarkGray))));
    lines.push(Line::from(PRESETS.iter().map(|c| Span::styled("   ", Style::new().bg(rgb_color(*c)))).collect::<Vec<_>>()));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("   ", Style::new().bg(rgb_color(cur))),
        Span::raw(format!(" #{}", hex_of(cur))),
        Span::raw(" ".repeat((OK_X as usize).saturating_sub(13))),
        Span::styled("[ OK ]", Style::new().fg(Color::Black).bg(Color::Green)),
        Span::raw(" "),
        Span::styled("[ Cancel ]", Style::new().fg(Color::Black).bg(Color::Red)),
    ]));
    lines.push(Line::from(Span::styled("arrows/0-9 adjust   Tab next colour   + - add/remove   p preset", Style::new().fg(Color::DarkGray))));
    lines.push(Line::from(Span::styled("Enter OK   Esc cancel", Style::new().fg(Color::DarkGray))));
    f.render_widget(
        Paragraph::new(localize(lines)).block(Block::bordered().title(tr(" colour picker ").into_owned()).border_style(Style::new().fg(Color::Cyan))),
        r,
    );
}

impl App {
    pub(crate) fn open_picker(&mut self) {
        let t = &self.tabs[self.cur];
        let row = &t.rows[t.sel];
        if let Value::Text(text) = &row.value {
            self.modal.picker = Some(Picker {
                tab: self.cur,
                row: t.sel,
                colors: parse_color_list(text),
                idx: 0,
                chan: 0,
                max: if t.id == TabId::Dpi { 8 } else { 7 },
                typed: String::new(),
            });
        }
    }

    /// Open the colour picker on this page's colour row, starting on colour number `color_idx`.
    pub(crate) fn open_picker_at(&mut self, color_idx: usize) {
        let t = &self.tabs[self.cur];
        if let Some(row) = t.rows.iter().position(is_color_row) {
            if let Value::Text(text) = &t.rows[row].value {
                let mut colors = parse_color_list(text);
                let max = if t.id == TabId::Dpi { 8 } else { 7 };
                while colors.len() <= color_idx.min(max - 1) {
                    colors.push(*colors.last().unwrap());
                }
                self.modal.picker = Some(Picker { tab: self.cur, row, colors, idx: color_idx.min(max - 1), chan: 0, max, typed: String::new() });
            }
        }
    }

    pub(crate) fn picker_close(&mut self, ok: bool) {
        if let Some(p) = self.modal.picker.take() {
            if ok {
                let text = p.colors.iter().map(|c| hex_of(*c)).collect::<Vec<_>>().join(",");
                self.tabs[p.tab].rows[p.row].value = Value::Text(text);
            }
        }
    }

    pub(crate) fn picker_key(&mut self, k: KeyEvent) {
        let Some(p) = self.modal.picker.as_mut() else { return };
        if !matches!(k.code, KeyCode::Char('0'..='9') | KeyCode::Backspace) {
            p.typed.clear();
        }
        match k.code {
            KeyCode::Char(d @ '0'..='9') => {
                p.typed.push(d);
                if p.typed.parse::<u32>().unwrap_or(0) > 255 {
                    p.typed = d.to_string();
                }
                p.colors[p.idx][p.chan] = p.typed.parse().unwrap_or(0);
            }
            KeyCode::Backspace => {
                p.typed.pop();
                p.colors[p.idx][p.chan] = p.typed.parse().unwrap_or(0);
            }
            KeyCode::Esc | KeyCode::Char('q') => self.picker_close(false),
            KeyCode::Enter => self.picker_close(true),
            KeyCode::Up => p.chan = (p.chan + 2) % 3,
            KeyCode::Down => p.chan = (p.chan + 1) % 3,
            KeyCode::Left | KeyCode::Right | KeyCode::PageUp | KeyCode::PageDown => {
                let d: i32 = if matches!(k.code, KeyCode::Left | KeyCode::PageDown) { -1 } else { 1 };
                let step = if matches!(k.code, KeyCode::PageUp | KeyCode::PageDown) { 16 } else { 1 };
                let c = &mut p.colors[p.idx][p.chan];
                *c = (*c as i32 + d * step).clamp(0, 255) as u8;
            }
            KeyCode::Char(c @ ('p' | 'P')) => {
                let n = PRESETS.len();
                let at = PRESETS.iter().position(|x| *x == p.colors[p.idx]);
                let next = match (at, c) {
                    (Some(i), 'p') => (i + 1) % n,
                    (Some(i), _) => (i + n - 1) % n,
                    (None, 'p') => 0,
                    (None, _) => n - 1,
                };
                p.colors[p.idx] = PRESETS[next];
            }
            KeyCode::Tab => p.idx = (p.idx + 1) % p.colors.len(),
            KeyCode::BackTab => p.idx = (p.idx + p.colors.len() - 1) % p.colors.len(),
            KeyCode::Char('+') if p.colors.len() < p.max => {
                let last = p.colors[p.idx];
                p.colors.insert(p.idx + 1, last);
                p.idx += 1;
            }
            KeyCode::Char('-') if p.colors.len() > 1 => {
                p.colors.remove(p.idx);
                p.idx = p.idx.min(p.colors.len() - 1);
            }
            _ => {}
        }
    }

    pub(crate) fn picker_mouse(&mut self, m: MouseEvent) {
        let r = picker_rect(self.area);
        let (col, row) = (m.column, m.row);
        if m.kind == MouseEventKind::Down(MouseButton::Left) && (col < r.x || row < r.y || col >= r.x + r.width || row >= r.y + r.height) {
            self.picker_close(false);
            return;
        }
        if col <= r.x || row <= r.y || col + 1 >= r.x + r.width || row + 1 >= r.y + r.height {
            return;
        }
        let (off, line) = (col - r.x - 1, row - r.y - 1);
        let down = m.kind == MouseEventKind::Down(MouseButton::Left);
        let drag = matches!(m.kind, MouseEventKind::Drag(MouseButton::Left));
        let scroll = match m.kind {
            MouseEventKind::ScrollUp => 8i32,
            MouseEventKind::ScrollDown => -8,
            _ => 0,
        };
        let mut result: Option<bool> = None;
        {
            let Some(p) = self.modal.picker.as_mut() else { return };
            p.typed.clear();
            if (down || drag) && (L_PAL..L_PAL + PAL_ROWS).contains(&line) && off < PAL_W {
                p.colors[p.idx] = palette_color(off, line - L_PAL);
            } else if (down || drag) && line == L_GREY && off < PAL_W {
                let v = (off as u32 * 255 / (PAL_W as u32 - 1)) as u8;
                p.colors[p.idx] = [v, v, v];
            } else if (L_BARS..L_BARS + 3).contains(&line) {
                let ch = (line - L_BARS) as usize;
                if down || drag {
                    p.chan = ch;
                    let x = off.saturating_sub(BAR_X).min(BAR_W - 1);
                    p.colors[p.idx][ch] = (x as u32 * 255 / (BAR_W as u32 - 1)) as u8;
                } else if scroll != 0 {
                    p.chan = ch;
                    let c = &mut p.colors[p.idx][ch];
                    *c = (*c as i32 + scroll).clamp(0, 255) as u8;
                }
            } else if down && line == L_PRESETS {
                if let Some(c) = PRESETS.get((off / 3) as usize) {
                    p.colors[p.idx] = *c;
                }
            } else if down && line == L_CHIPS {
                let n = p.colors.len() as u16;
                let end = 7 + 4 * n;
                if off >= 7 && off < end {
                    p.idx = ((off - 7) / 4) as usize;
                } else if off > end && off < end + 6 {
                    if p.colors.len() < p.max {
                        let last = p.colors[p.idx];
                        p.colors.insert(p.idx + 1, last);
                        p.idx += 1;
                    }
                } else if off >= end + 7 && off < end + 12 && p.colors.len() > 1 {
                    p.colors.remove(p.idx);
                    p.idx = p.idx.min(p.colors.len() - 1);
                }
            } else if down && line == L_BUTTONS {
                if (OK_X..OK_X + 6).contains(&off) {
                    result = Some(true);
                } else if (CANCEL_X..CANCEL_X + 10).contains(&off) {
                    result = Some(false);
                }
            }
        }
        if let Some(ok) = result {
            self.picker_close(ok);
        }
    }
}
