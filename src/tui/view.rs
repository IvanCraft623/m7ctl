//! Drawing: layout, rows and the page chrome.

use crate::tui::*;

/// Width of the label column; values start right after it.
pub(crate) const LABEL_W: usize = 30;

pub(crate) const VALUE_X: u16 = 2 + LABEL_W as u16;

/// First profile row on the profiles page (lines before it: status, auto-save, gap, buttons, gap, header).
pub(crate) const P_FIRST: usize = 6;

pub(crate) fn choice_text(row: &Row, i: usize, editing: bool) -> String {
    if is_custom_choice(row) {
        if editing {
            tr("< custom: {}_ >").replacen("{}", &row.extra, 1)
        } else if row.extra.is_empty() {
            tr("< custom... >").into_owned()
        } else {
            tr("< custom: {} >").replacen("{}", &row.extra, 1)
        }
    } else {
        format!("< {} >", tr(&row.opts[i]))
    }
}

/// Width in cells of the `< value >` text of a row, for locating clicks on its arrows.
pub(crate) fn value_width(row: &Row) -> u16 {
    (match &row.value {
        Value::Choice(i) => choice_text(row, *i, false).chars().count(),
        Value::Num(n) => format!("< {n} >").chars().count(),
        Value::Bool(b) => format!("< {} >", tr(if *b { "on" } else { "off" })).chars().count(),
        _ => 0,
    }) as u16
}

pub(crate) fn areas(area: Rect) -> [Rect; 3] {
    let c = Layout::vertical([Constraint::Length(3), Constraint::Min(8), Constraint::Length(5)]).split(area);
    [c[0], c[1], c[2]]
}

pub(crate) fn tab_label(i: usize, dirty: bool) -> String {
    format!(" {}{} ", tr(TABS[i].1), if dirty { '*' } else { ' ' })
}

/// Horizontal extent of each page title inside the title bar (relative to its inner left edge).
pub(crate) fn tab_ranges() -> Vec<(u16, u16)> {
    let mut x = 0u16;
    TABS.iter()
        .enumerate()
        .map(|(i, _)| {
            let w = tab_label(i, false).chars().count() as u16;
            let r = (x, x + w);
            x += w + 1; // one separator
            r
        })
        .collect()
}

pub(crate) fn fit(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n <= w {
        format!("{s:<w$}")
    } else {
        let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
        t.push('~');
        t
    }
}

/// `avail` is how many cells are left for the value and its tag.
/// How a row is drawn: selected, being edited, dimmed (does not apply to the current mode), changed but not applied.
pub(crate) struct RowState {
    pub(crate) selected: bool,
    pub(crate) editing: bool,
    pub(crate) dim: bool,
    pub(crate) dirty: bool,
    /// Digits typed so far into this numeric row.
    pub(crate) typed: Option<String>,
}

pub(crate) fn row_line(row: &Row, state: RowState, chip: Option<[u8; 3]>, tag: &str, avail: usize) -> Line<'static> {
    let RowState { selected, editing, dim, dirty, typed } = state;
    let sel = if selected {
        Style::new().fg(Color::Black).bg(Color::Cyan)
    } else if dim {
        Style::new().fg(Color::DarkGray)
    } else {
        Style::new()
    };
    let mark = format!("{}{}", if selected { '>' } else { ' ' }, if dirty { '*' } else { ' ' });
    let mark_style = if dirty && !selected { Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { sel };
    match &row.value {
        Value::Action => Line::from(Span::styled(format!("{mark}[ {} ]", tr(&row.label)), if selected { sel } else { Style::new().fg(Color::Green) })),
        v => {
            let is_col = is_color_row(row);
            let valid = if let Value::Text(t) = v { colors_valid(t) } else { true };
            let shown = if let (Some(t), true) = (&typed, selected && matches!(v, Value::Num(_) | Value::Choice(_))) {
                format!("< {t}_ >")
            } else {
                match v {
                    Value::Choice(i) => choice_text(row, *i, editing && selected),
                    Value::Num(n) => format!("< {n} >"),
                    Value::Bool(b) => format!("< {} >", tr(if *b { "on" } else { "off" })),
                    Value::Text(t) => {
                        if row.label == "2. Steps" && !(editing && selected) {
                            ellipsize(&describe_steps(t), avail.saturating_sub(tag.chars().count() + 2))
                        } else if is_col && valid && !(editing && selected) {
                            String::new()
                        } else {
                            format!("{t}{}", if editing && selected { "_" } else { "" })
                        }
                    }
                    Value::Action => String::new(),
                }
            };
            let bold = if selected { sel.add_modifier(Modifier::BOLD) } else { Style::new().add_modifier(Modifier::BOLD) };
            let mut spans = vec![Span::styled(mark, mark_style), Span::styled(fit(&tr(&row.label), LABEL_W), sel)];
            if chip.is_some() || !tag.is_empty() {
                spans.push(Span::styled(format!("{shown:<14}"), bold));
                if let Some(c) = chip {
                    spans.push(Span::styled("    ", Style::new().bg(rgb_color(c))));
                }
                if !tag.is_empty() {
                    spans.push(Span::styled(format!(" {}", tr(tag)), Style::new().fg(Color::Yellow)));
                }
            } else {
                spans.push(Span::styled(shown, bold));
            }
            if let Value::Text(t) = v {
                if is_col {
                    if valid {
                        if !(editing && selected) {
                            // chips shrink (6 to 1 cells wide) so the whole list fits the row
                            let list = parse_color_list(t);
                            let width = (avail / list.len().max(1)).saturating_sub(1).clamp(1, 6);
                            for c in &list {
                                spans.push(Span::styled(" ".repeat(width), Style::new().bg(rgb_color(*c))));
                                spans.push(Span::raw(" "));
                            }
                            let hex = format!(" {}", t.split(',').map(|c| format!("#{}", c.trim().trim_start_matches('#'))).collect::<Vec<_>>().join(" "));
                            if selected && list.len() * (width + 1) + hex.chars().count() <= avail {
                                spans.push(Span::styled(hex, Style::new().fg(Color::DarkGray)));
                            }
                        }
                    } else {
                        spans.push(Span::styled("  (needs 6-digit hex colours like ff8000, comma separated)", Style::new().fg(Color::Red)));
                    }
                }
            }
            Line::from(spans)
        }
    }
}

pub(crate) fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < 80 || area.height < 24 {
        f.render_widget(Paragraph::new(tr("The terminal is too small: please enlarge it (at least 80x24).").into_owned()), area);
        return;
    }
    let chunks = areas(area);
    let mut spans: Vec<Span> = vec![];
    for i in 0..TABS.len() {
        let style = if i == app.cur { Style::new().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::new() };
        spans.push(Span::styled(tab_label(i, app.tabs[i].dirty()), style));
        if i + 1 < TABS.len() {
            spans.push(Span::raw("\u{2502}"));
        }
    }
    let bat = if app.live.status.battery.is_empty() { String::new() } else { tr(" | battery {}% ").replacen("{}", &app.live.status.battery, 1) };
    let prof = match &app.prof.active {
        Some(a) => tr(if app.prof.modified { " | profile {} (unsaved) " } else { " | profile {} " }).replacen("{}", a, 1),
        None => String::new(),
    };
    let title = format!(" Machenike M7 Pro{bat}{prof} ");
    f.render_widget(Paragraph::new(Line::from(spans)).block(Block::bordered().title(title)), chunks[0]);
    if app.modal.chooser.is_some() {
        f.render_widget(Clear, Rect::new(area.x, chunks[1].y, area.width, area.height.saturating_sub(chunks[1].y)));
        draw_chooser(f, app, area);
        return;
    }
    if let Some(r) = &app.modal.recording {
        f.render_widget(Clear, Rect::new(area.x, chunks[1].y, area.width, area.height.saturating_sub(chunks[1].y)));
        draw_recording(f, r, area);
        return;
    }
    if let Some(reasons) = &app.modal.quit_prompt {
        f.render_widget(Clear, Rect::new(area.x, chunks[1].y, area.width, area.height.saturating_sub(chunks[1].y)));
        draw_quit(f, app, area, reasons);
        return;
    }
    if app.modal.help {
        f.render_widget(Clear, Rect::new(area.x, chunks[1].y, area.width, area.height.saturating_sub(chunks[1].y)));
        draw_help(f, area);
        return;
    }
    if let Some((title, typed)) = app.prompt_info() {
        f.render_widget(Clear, Rect::new(area.x, chunks[1].y, area.width, area.height.saturating_sub(chunks[1].y)));
        draw_prompt(f, area, &title, &typed);
        return;
    }
    if app.modal.picker.is_some() {
        // modal: blank everything below the title bar (all of it when the picker is too tall to leave the bar visible)
        let top = if picker_rect(area).y < chunks[1].y { area.y } else { chunks[1].y };
        f.render_widget(Clear, Rect::new(area.x, top, area.width, area.height.saturating_sub(top)));
        draw_picker(f, app, area);
        return;
    }
    draw_body(f, app, chunks[1]);
    let busy = if app.bg.busy { " - working..." } else { "" };
    let keep = (chunks[2].height as usize).saturating_sub(3).max(1);
    let skip = app.log.len().saturating_sub(keep);
    let lines: Vec<Line> = app.log[skip..]
        .iter()
        .map(|l| {
            let style = if l.starts_with("$ ") {
                Style::new().fg(Color::Cyan)
            } else if l.starts_with("error") || l.contains("FAILED") || l.contains("NOT applied") || l.contains("stayed asleep") {
                Style::new().fg(Color::Red)
            } else if l.contains(": done") || l.contains("loaded") || l.contains("saved") {
                Style::new().fg(Color::Green)
            } else {
                Style::new()
            };
            Line::from(Span::styled(l.clone(), style))
        })
        .collect();
    let log_title = format!("{}{}", tr(" log ").trim_end(), tr(busy));
    f.render_widget(Paragraph::new(localize(lines)).block(Block::bordered().title(format!("{log_title} "))).wrap(Wrap { trim: false }), chunks[2]);
    draw_footer(f, app, area);
}

pub(crate) fn draw_body(f: &mut Frame, app: &App, area: Rect) {
    let tab = app.tab();
    let block = Block::bordered().title(format!(" {} ", tr(TABS[app.cur].1)));
    let mut lines: Vec<Line> = vec![];
    match tab.id {
        TabId::Status => lines = status_page(app),
        TabId::Profiles => lines = profile_page(app, area.height.saturating_sub(2) as usize),
        _ => {
            for (i, r) in tab.rows.iter().enumerate() {
                let light_mode = match tab.rows.first().map(|r| &r.value) {
                    Some(Value::Choice(m)) if tab.id == TabId::Lights => MODES[*m],
                    _ => "",
                };
                let unused = (tab.id == TabId::Dpi
                    && match (&tab.rows[0].value, r.label.strip_prefix("Stage ").and_then(|x| x.split(' ').next()).and_then(|x| x.parse::<i64>().ok())) {
                        (Value::Num(n), Some(k)) => k > *n,
                        _ => false,
                    })
                    || (tab.id == TabId::Lights
                        && ((light_mode == "off" && r.label.starts_with("Colour")) || (light_mode != "cycle" && r.label.starts_with("Cycle speed"))));
                let (chip, tag) = if tab.id == TabId::Macros && r.label == "1. Slot" {
                    let slot = match r.value {
                        Value::Num(n) => n,
                        _ => 1,
                    };
                    (None, slot_status(&app.cur_state, slot))
                } else if tab.id == TabId::Macros && r.label == "2. Steps" {
                    (None, if let Value::Text(t) = &r.value { steps_tag(t) } else { String::new() })
                } else if tab.id == TabId::Dpi {
                    stage_decor(tab, r)
                } else if r.label.starts_with("Lift-off") {
                    (None, "not tested on the mouse yet".to_string())
                } else {
                    (None, String::new())
                };
                lines.push(row_line(
                    r,
                    RowState {
                        selected: i == tab.sel,
                        editing: app.editing || app.modal.editing_custom,
                        dim: unused,
                        dirty: tab.dirty_row(i),
                        typed: if i == tab.sel { app.typing.clone() } else { None },
                    },
                    chip,
                    &tag,
                    (area.width as usize).saturating_sub(4 + VALUE_X as usize),
                ));
            }
            if tab.id == TabId::Macros {
                lines.push(Line::from(""));
                if tab.rows.iter().any(|r| r.label == "Repeat" && r.value == Value::Choice(0)) {
                    lines.push(Line::from(Span::styled("loop never stops by itself: clear the slot to end it", Style::new().fg(Color::Yellow))));
                } else {
                    lines.push(Line::from(Span::styled(
                        "Record keys or add steps, save them, then give the macro to a button.",
                        Style::new().fg(Color::DarkGray),
                    )));
                }
            }
            if tab.id == TabId::Buttons {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Enter or click: pick a function from a list (type to filter). \"custom...\": type your own",
                    Style::new().fg(Color::DarkGray),
                )));
            }
        }
    }
    f.render_widget(Paragraph::new(localize(lines)).block(block), area);
}
