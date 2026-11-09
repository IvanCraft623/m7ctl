//! One-line text prompt (profile names, macro steps) and the clickable key hints under the log.

use crate::tui::*;

const OK: &str = "[ OK ]";
const CANCEL: &str = "[ Cancel ]";

pub(crate) fn prompt_rect(area: Rect) -> Rect {
    let w = 64u16.min(area.width);
    let h = 5u16.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

pub(crate) fn draw_prompt(f: &mut Frame, area: Rect, title: &str, typed: &str) {
    let r = prompt_rect(area);
    f.render_widget(Clear, r);
    let lines = vec![
        Line::from(format!("{typed}_")),
        Line::from(""),
        Line::from(vec![
            Span::styled(OK, Style::new().fg(Color::Black).bg(Color::Green)),
            Span::raw("  "),
            Span::styled(CANCEL, Style::new().fg(Color::Black).bg(Color::Red)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).block(Block::bordered().title(format!(" {title} ")).border_style(Style::new().fg(Color::Yellow))), r);
}

/// The keys shown under the log; each one can also be clicked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Hint {
    Pages,
    Apply,
    Undo,
    Refresh,
    Import,
    Edit,
    Popup,
    Language,
    Help,
    Quit,
}

impl Hint {
    fn key(self) -> &'static str {
        match self {
            Hint::Pages => "Tab",
            Hint::Apply => "a",
            Hint::Undo => "u",
            Hint::Refresh => "r",
            Hint::Import => "i",
            Hint::Edit => "e",
            Hint::Popup => "p",
            Hint::Language => "L",
            Hint::Help => "?",
            Hint::Quit => "q",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Hint::Pages => "pages",
            Hint::Apply => "apply",
            Hint::Undo => "undo",
            Hint::Refresh => "refresh",
            Hint::Import => "import",
            Hint::Edit => "edit",
            Hint::Popup => "popup",
            Hint::Language => "language",
            Hint::Help => "help",
            Hint::Quit => "quit",
        }
    }

    pub(crate) fn text(self) -> String {
        format!("{} {}", self.key(), tr(self.label()))
    }
}

const HINT_GAP: u16 = 3;

/// Where the hints go: the last line inside the log box.
pub(crate) fn footer_rect(area: Rect) -> Rect {
    let log = areas(area)[2];
    Rect::new(log.x + 1, log.y + log.height.saturating_sub(2), log.width.saturating_sub(2), 1)
}

pub(crate) fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![];
    let hints = app.footer_hints();
    if hints.is_empty() {
        let text = if app.typing.is_some() { "type a number, Enter confirms, Esc cancels" } else { "type the function (e.g. key:ctrl+c), Enter when done" };
        spans.push(Span::styled(tr(text).into_owned(), Style::new().fg(Color::DarkGray)));
    }
    for h in hints {
        spans.push(Span::styled(h.key(), Style::new().fg(Color::Cyan)));
        spans.push(Span::styled(format!(" {}", tr(h.label())), Style::new().fg(Color::DarkGray)));
        spans.push(Span::raw(" ".repeat(HINT_GAP as usize)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), footer_rect(area));
}

impl App {
    /// Title and typed text of the open prompt, if any.
    pub(crate) fn prompt_info(&self) -> Option<(String, String)> {
        if let Some(typed) = &self.modal.naming {
            let title = match &self.modal.naming_kind {
                NameKind::SaveAs => tr("Name for the new profile").into_owned(),
                NameKind::Rename(old) => tr(&format!("New name for '{old}'")).into_owned(),
                NameKind::ImportPath => tr("Path of the .psf file").into_owned(),
            };
            return Some((title, typed.clone()));
        }
        self.modal.step_prompt.as_ref().map(|p| (tr(p.label()).into_owned(), p.text.clone()))
    }

    pub(crate) fn prompt_mouse(&mut self, m: MouseEvent) {
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let r = prompt_rect(self.area);
        let (col, row) = (m.column, m.row);
        if col <= r.x || col + 1 >= r.x + r.width || row != r.y + 3 {
            return;
        }
        let off = col - r.x - 1;
        let ok = OK.chars().count() as u16;
        if off < ok {
            if self.modal.naming.is_some() {
                self.finish_naming();
            } else {
                self.finish_step_prompt();
            }
        } else if (ok + 2..ok + 2 + CANCEL.chars().count() as u16).contains(&off) {
            self.modal.naming = None;
            self.modal.step_prompt = None;
        }
    }

    pub(crate) fn footer_hints(&self) -> Vec<Hint> {
        if self.modal.editing_custom || self.typing.is_some() {
            return vec![];
        }
        let mut v = vec![Hint::Pages];
        match self.tab().id {
            TabId::Status => v.extend([Hint::Refresh, Hint::Popup]),
            TabId::Profiles => v.push(Hint::Import),
            _ => {
                v.extend([Hint::Apply, Hint::Undo]);
                if self.selected_editable() {
                    v.push(Hint::Edit);
                }
            }
        }
        v.extend([Hint::Language, Hint::Help, Hint::Quit]);
        v
    }

    /// A click `col` cells into the hint line.
    pub(crate) fn footer_click(&mut self, col: u16) {
        let mut x = 0u16;
        for h in self.footer_hints() {
            let w = h.text().chars().count() as u16;
            if col >= x && col < x + w {
                self.run_hint(h);
                return;
            }
            x += w + HINT_GAP;
        }
    }

    fn run_hint(&mut self, h: Hint) {
        match h {
            Hint::Pages => self.cur = (self.cur + 1) % self.tabs.len(),
            Hint::Apply => self.apply_selected(),
            Hint::Undo => self.revert_page(),
            Hint::Refresh => self.refresh_status(),
            Hint::Import => self.start_naming(NameKind::ImportPath),
            Hint::Edit => self.key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
            Hint::Popup => self.toggle_popup(),
            Hint::Language => self.open_language_chooser(),
            Hint::Help => self.modal.help = true,
            Hint::Quit => self.request_quit(),
        }
    }
}
