//! Filterable list (Buttons page, add-a-step menu).

use crate::tui::i18n::Lang;
use crate::tui::*;

/// A list to pick from, with type-to-filter (used for the button functions).
#[derive(Clone, Debug)]
pub(crate) struct Chooser {
    pub(crate) row: usize,
    pub(crate) filter: String,
    pub(crate) sel: usize,
    /// Own options (and title) instead of the selected row's: used by "Add a step".
    pub(crate) items: Option<(String, Vec<String>)>,
    /// The options are languages (picking one changes the interface language).
    pub(crate) lang: bool,
}

/// The box fits its `total` options (at most 20 rows, at least 6).
pub(crate) fn chooser_rect(area: Rect, total: usize) -> Rect {
    let w = 40u16.min(area.width);
    let h = (total as u16 + 4).clamp(6, 20).min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

/// How many options the chooser shows at once (its box minus borders, the filter line and the hint line).
pub(crate) fn chooser_visible(area: Rect, total: usize) -> usize {
    (chooser_rect(area, total).height as usize).saturating_sub(4)
}

pub(crate) fn chooser_top(sel: usize, visible: usize) -> usize {
    if sel < visible {
        0
    } else {
        sel + 1 - visible
    }
}

pub(crate) fn draw_chooser(f: &mut Frame, app: &App, area: Rect) {
    let Some(c) = &app.modal.chooser else { return };
    let total = app.chooser_total();
    let r = chooser_rect(area, total);
    f.render_widget(Clear, r);
    let items = app.chooser_items();
    let visible = chooser_visible(area, total);
    let top = chooser_top(c.sel, visible);
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines = vec![Line::from(vec![Span::styled("Filter: ", dim), Span::raw(format!("{}_", c.filter))])];
    if items.is_empty() {
        lines.push(Line::from(Span::styled("  no match", dim)));
    }
    for (pos, (_, label)) in items.iter().enumerate().skip(top).take(visible) {
        let style = if pos == c.sel { Style::new().fg(Color::Black).bg(Color::Cyan) } else { Style::new() };
        // the add-a-step menu is text; the function list shows command names, which stay as they are
        let shown = if c.items.is_some() { tr(label).into_owned() } else { label.clone() };
        lines.push(Line::from(Span::styled(format!(" {shown:<width$}", width = r.width as usize - 4), style)));
    }
    while lines.len() < visible + 1 {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(if c.items.is_some() { "Enter: choose   Esc: cancel" } else { "Enter: choose and apply   Esc: cancel" }, dim)));
    let title = match &c.items {
        Some((t, _)) => format!(" {} ", tr(t)),
        None => format!(" {} ", tr(&app.tabs[app.cur].rows[c.row].label)),
    };
    let close = Line::from(Span::styled(" x ", Style::new().fg(Color::Red))).right_aligned();
    f.render_widget(Paragraph::new(localize(lines)).block(Block::bordered().title(title).title(close).border_style(Style::new().fg(Color::Cyan))), r);
}

impl App {
    /// How many options the open chooser has before filtering.
    pub(crate) fn chooser_total(&self) -> usize {
        let Some(c) = &self.modal.chooser else { return 0 };
        match &c.items {
            Some((_, v)) => v.len(),
            None => self.tabs[self.cur].rows[c.row].opts.len(),
        }
    }

    /// Options of the chooser's row that match the filter: (index into the row's options, label).
    pub(crate) fn chooser_items(&self) -> Vec<(usize, String)> {
        let Some(c) = &self.modal.chooser else { return vec![] };
        let f = c.filter.to_lowercase();
        c.items
            .as_ref()
            .map(|(_, v)| v)
            .unwrap_or_else(|| &self.tabs[self.cur].rows[c.row].opts)
            .iter()
            .enumerate()
            .filter(|(_, o)| o.to_lowercase().contains(&f))
            .map(|(i, o)| (i, o.clone()))
            .collect()
    }

    pub(crate) fn open_chooser(&mut self) {
        let t = &self.tabs[self.cur];
        let row = t.sel;
        let sel = match t.rows[row].value {
            Value::Choice(i) => i,
            _ => 0,
        };
        self.modal.chooser = Some(Chooser { row, filter: String::new(), sel, items: None, lang: false });
    }

    /// The language menu (the `L` key and the language hint).
    pub(crate) fn open_language_chooser(&mut self) {
        let names = Lang::ALL.iter().map(|l| l.name().to_string()).collect();
        let sel = Lang::ALL.iter().position(|l| *l == crate::tui::i18n::lang()).unwrap_or(0);
        self.modal.chooser = Some(Chooser { row: self.tabs[self.cur].sel, filter: String::new(), sel, items: Some(("Language".into(), names)), lang: true });
    }

    /// Take the highlighted option, then apply it (or start typing a custom function).
    pub(crate) fn chooser_pick(&mut self) {
        let items = self.chooser_items();
        let Some(c) = self.modal.chooser.take() else { return };
        let Some((idx, label)) = items.get(c.sel) else { return };
        if c.lang {
            self.choose_language(Lang::ALL[*idx]);
            return;
        }
        if c.items.is_some() {
            let label = label.clone();
            self.add_step_pick(&label);
            return;
        }
        self.tabs[self.cur].rows[c.row].value = Value::Choice(*idx);
        self.tabs[self.cur].sel = c.row;
        if is_custom_choice(&self.tabs[self.cur].rows[c.row]) {
            self.modal.editing_custom = true;
        } else {
            self.apply_selected();
        }
    }

    pub(crate) fn chooser_key(&mut self, k: KeyEvent) {
        let n = self.chooser_items().len();
        let page = chooser_visible(self.area, self.chooser_total()).max(1);
        let Some(c) = self.modal.chooser.as_mut() else { return };
        match k.code {
            KeyCode::Esc => self.modal.chooser = None,
            KeyCode::Enter => self.chooser_pick(),
            KeyCode::Up => c.sel = c.sel.saturating_sub(1),
            KeyCode::Down => c.sel = (c.sel + 1).min(n.saturating_sub(1)),
            KeyCode::PageUp => c.sel = c.sel.saturating_sub(page),
            KeyCode::PageDown => c.sel = (c.sel + page).min(n.saturating_sub(1)),
            KeyCode::Home => c.sel = 0,
            KeyCode::End => c.sel = n.saturating_sub(1),
            KeyCode::Backspace => {
                c.filter.pop();
                c.sel = 0;
            }
            KeyCode::Char(ch) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                c.filter.push(ch);
                c.sel = 0;
            }
            _ => {}
        }
    }

    pub(crate) fn chooser_mouse(&mut self, m: MouseEvent) {
        let n = self.chooser_items().len();
        let total = self.chooser_total();
        let r = chooser_rect(self.area, total);
        let visible = chooser_visible(self.area, total);
        let Some(c) = self.modal.chooser.as_mut() else { return };
        match m.kind {
            MouseEventKind::ScrollUp => c.sel = c.sel.saturating_sub(1),
            MouseEventKind::ScrollDown => c.sel = (c.sel + 1).min(n.saturating_sub(1)),
            MouseEventKind::Down(MouseButton::Left) => {
                let (col, row) = (m.column, m.row);
                let outside = col < r.x || row < r.y || col >= r.x + r.width || row >= r.y + r.height;
                let close_button = row == r.y && col + 4 >= r.x + r.width && col + 1 < r.x + r.width;
                if outside || close_button {
                    self.modal.chooser = None;
                    return;
                }
                if col <= r.x || row <= r.y || col + 1 >= r.x + r.width || row + 1 >= r.y + r.height {
                    return;
                }
                let line = (row - r.y - 1) as usize;
                if line >= 1 && line <= visible {
                    let top = chooser_top(c.sel, visible);
                    let idx = top + line - 1;
                    if idx < n {
                        c.sel = idx;
                        self.chooser_pick();
                    }
                }
            }
            _ => {}
        }
    }
}
