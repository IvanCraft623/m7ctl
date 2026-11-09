//! Quit prompt and help screen.

use crate::tui::*;

pub(crate) fn quit_rect(area: Rect, reasons: usize) -> Rect {
    let w = 74u16.min(area.width);
    let h = (reasons as u16 + 4).min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

/// Buttons of the quit prompt: (label, id) where id 0 = quit anyway, 1 = save profile and quit, 2 = stay.
pub(crate) fn quit_buttons(profile_unsaved: bool) -> Vec<(String, u8)> {
    let mut v = vec![(tr(text::quit::QUIT).into_owned(), 0)];
    if profile_unsaved {
        v.push((tr(text::quit::SAVE_AND_QUIT).into_owned(), 1));
    }
    v.push((tr(text::quit::STAY).into_owned(), 2));
    v
}

pub(crate) fn draw_quit(f: &mut Frame, app: &App, area: Rect, reasons: &[String]) {
    let r = quit_rect(area, reasons.len());
    f.render_widget(Clear, r);
    let mut lines: Vec<Line> = reasons.iter().map(|t| Line::from(Span::styled(t.clone(), Style::new().fg(Color::Yellow)))).collect();
    lines.push(Line::from(""));
    let mut spans = vec![];
    for (label, id) in quit_buttons(app.profile_unsaved()) {
        let style = match id {
            0 => Style::new().fg(Color::Black).bg(Color::Red),
            1 => Style::new().fg(Color::Black).bg(Color::Green),
            _ => Style::new().fg(Color::Black).bg(Color::Cyan),
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("  "));
    }
    lines.push(Line::from(spans));
    f.render_widget(
        Paragraph::new(localize(lines)).block(Block::bordered().title(tr(text::quit::TITLE).into_owned()).border_style(Style::new().fg(Color::Yellow))),
        r,
    );
}

pub(crate) fn draw_help(f: &mut Frame, area: Rect) {
    let w = 66u16.min(area.width);
    let h = 22u16.min(area.height);
    let r = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, r);
    let head = |t: &str| Line::from(Span::styled(t.to_string(), Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines = vec![head(text::help::KEYS)];
    lines.extend(text::help::KEY_LINES.iter().map(|l| Line::from(*l)));
    lines.extend([Line::from(""), head(text::help::PROFILES)]);
    lines.extend(text::help::PROFILE_LINES.iter().map(|l| Line::from(*l)));
    lines.extend([Line::from(""), head(text::help::MOUSE), Line::from(text::help::MOUSE_LINE), Line::from(""), head(text::help::MARKERS)]);
    lines.push(Line::from(vec![
        Span::styled(text::help::MARKER_STAR, Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(text::help::MARKER_TEXT),
    ]));
    lines.extend([Line::from(""), Line::from(Span::styled(text::help::CLOSE, dim))]);
    f.render_widget(
        Paragraph::new(localize(lines)).block(Block::bordered().title(tr(text::help::TITLE).into_owned()).border_style(Style::new().fg(Color::Cyan))),
        r,
    );
}

impl App {
    /// Reasons to ask before quitting (empty: just quit).
    pub(crate) fn quit_reasons(&self) -> Vec<String> {
        let mut v = vec![];
        let dirty: Vec<String> = self.tabs.iter().enumerate().filter(|(_, t)| t.dirty()).map(|(i, _)| tr(TABS[i].1).into_owned()).collect();
        if !dirty.is_empty() {
            v.push(text::quit::not_applied(&dirty.join(", ")));
        }
        if let (Some(a), true) = (&self.prof.active, self.prof.modified && !self.prof.autosave) {
            v.push(text::quit::unsaved_profile(a));
        }
        v
    }

    pub(crate) fn request_quit(&mut self) {
        let r = self.quit_reasons();
        if r.is_empty() {
            self.quit = true;
        } else {
            self.modal.quit_prompt = Some(r);
        }
    }

    /// True when this click repeats the previous one on the same line within half a second.
    pub(crate) fn quit_mouse(&mut self, m: MouseEvent) {
        if !matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }
        let n = self.modal.quit_prompt.as_ref().map(|r| r.len()).unwrap_or(0);
        let r = quit_rect(self.area, n);
        let (col, row) = (m.column, m.row);
        if col <= r.x || row <= r.y || col + 1 >= r.x + r.width || row + 1 >= r.y + r.height {
            return;
        }
        let (off, line) = (col - r.x - 1, (row - r.y - 1) as usize);
        if line == n + 1 {
            let mut x = 0u16;
            for (label, id) in quit_buttons(self.profile_unsaved()) {
                let w = label.chars().count() as u16;
                if off >= x && off < x + w {
                    self.quit_choice(id);
                    return;
                }
                x += w + 2;
            }
        }
    }

    /// 0 = quit anyway, 1 = save the profile and quit, 2 = stay.
    pub(crate) fn quit_choice(&mut self, id: u8) {
        match id {
            0 => self.quit = true,
            1 => {
                if let Some(a) = &self.prof.active {
                    let _ = self.ctx.store.save_profile(a);
                }
                self.quit = true;
            }
            _ => self.modal.quit_prompt = None,
        }
    }
}
