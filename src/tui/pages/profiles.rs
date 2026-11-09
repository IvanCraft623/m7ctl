//! Profiles page.

use crate::tui::*;

/// The profiles page: what is active, how saving works, the list, and the selected profile's contents.
pub(crate) fn profile_page(app: &App, height: usize) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines: Vec<Line> = vec![];
    // 0: active profile
    lines.push(match &app.prof.active {
        Some(a) => {
            let mut v = vec![Span::raw("Active profile: "), Span::styled(a.clone(), Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan))];
            v.push(if app.prof.modified {
                Span::styled("   unsaved changes", Style::new().fg(Color::Yellow))
            } else {
                Span::styled("   saved", Style::new().fg(Color::Green))
            });
            Line::from(v)
        }
        None => Line::from(vec![Span::raw("Active profile: "), Span::styled("none", Style::new().fg(Color::Yellow)), Span::raw("")]),
    });
    // 1: auto-save
    lines.push(Line::from(vec![
        Span::styled(if app.prof.autosave { "[x] " } else { "[ ] " }, Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("Auto-save changes to the active profile"),
        Span::styled(if app.prof.active.is_none() { "  (needs an active profile)" } else { "" }, dim),
    ]));
    lines.push(Line::from(""));
    // 4: buttons
    let mut spans = vec![];
    for (label, enabled, id) in app.profile_buttons() {
        let style = if !enabled {
            dim
        } else if id == 4 {
            Style::new().fg(Color::Red)
        } else {
            Style::new().fg(Color::Green)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("  "));
    }
    lines.push(Line::from(spans));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Profiles   (i: import a vendor .psf file)", dim)));
    if app.prof.names.is_empty() {
        lines.push(Line::from("  none yet"));
    }
    for (i, n) in app.prof.names.iter().enumerate() {
        let sel = i == app.prof.sel;
        let is_active = app.prof.active.as_deref() == Some(n.as_str());
        let style = if sel { Style::new().fg(Color::Black).bg(Color::Cyan) } else { Style::new() };
        let mut v = vec![Span::styled(format!("{}{} {:<18}", if sel { ">" } else { " " }, if is_active { "*" } else { " " }, n), style)];
        v.push(Span::styled(format!(" {}", summary(&app.prof.info[i])), if sel { style } else { dim }));
        if is_active {
            v.push(Span::styled(
                if app.prof.modified { "  (active, unsaved)" } else { "  (active)" },
                Style::new().fg(if app.prof.modified { Color::Yellow } else { Color::Green }),
            ));
        }
        lines.push(Line::from(v));
    }
    // contents of the selected profile, as far as the page has room
    if let (Some(name), Some(st)) = (app.prof.names.get(app.prof.sel), app.prof.info.get(app.prof.sel)) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(format!("Contents of '{name}':"), Style::new().add_modifier(Modifier::BOLD))));
        for l in profile_contents(st) {
            if lines.len() >= height {
                break;
            }
            lines.push(l);
        }
    }
    lines
}

/// Readable lines (with colour squares) for a settings record.
pub(crate) fn profile_contents(st: &State) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let chips = |text: &str| -> Vec<Span<'static>> {
        let mut v = vec![];
        if colors_valid(text) {
            for c in parse_color_list(text) {
                v.push(Span::styled("    ", Style::new().bg(rgb_color(c))));
                v.push(Span::raw(" "));
            }
        }
        v
    };
    let mut out = vec![];
    for (k, a) in st {
        let arg = |i: usize| a.get(i).cloned().unwrap_or_default();
        let opt = |name: &str| a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned();
        let (label, mut spans): (String, Vec<Span<'static>>) = match k.split(':').next().unwrap_or("") {
            "dpi" => {
                ("DPI stages".into(), vec![Span::raw(format!("{}  (starts on stage {})", arg(0).replace(',', ", "), opt("--active").unwrap_or("1".into())))])
            }
            "dpi-colors" => ("Scroll-light colours".into(), chips(&arg(0))),
            "rate" => ("Polling rate".into(), vec![Span::raw(format!("{} Hz", arg(0)))]),
            "light" => {
                let mut v = vec![Span::raw(arg(0))];
                if arg(0) != "off" {
                    v.push(Span::raw("  "));
                    v.extend(chips(&arg(1)));
                    if let Some(b) = opt("--brightness") {
                        v.push(Span::styled(format!(" brightness {b}"), dim));
                    }
                    if let Some(sp) = opt("--speed") {
                        v.push(Span::styled(format!(" speed {sp}"), dim));
                    }
                }
                ("Lights".into(), v)
            }
            "power" => (
                "Power-down".into(),
                vec![Span::raw(format!("{} min{}", arg(0), if a.iter().any(|x| x == "lights-off-moving") { ", lights off while moving" } else { "" }))],
            ),
            "motion-wake" => ("Motion wake-up".into(), vec![Span::raw(arg(0))]),
            "lod" => ("Lift-off distance".into(), vec![Span::raw(format!("{} mm", arg(0)))]),
            "button" => (format!("{} button", k.trim_start_matches("button:")), vec![Span::raw(arg(1))]),
            "macro" => (format!("Macro slot {}", arg(0)), vec![Span::raw(arg(1))]),
            _ => (k.clone(), vec![Span::raw(a.join(" "))]),
        };
        let mut line = vec![Span::styled(format!("  {:<22}", tr(&label)), dim)];
        line.append(&mut spans);
        out.push(Line::from(line));
    }
    out
}

impl App {
    pub(crate) fn refresh_profiles(&mut self) {
        self.prof.names = self.ctx.store.list_profiles();
        self.prof.info = self.prof.names.iter().map(|n| self.ctx.store.read_profile(n).unwrap_or_default()).collect();
        self.prof.active = self.ctx.store.active_profile();
        self.prof.modified = self.prof.active.as_deref().and_then(|n| self.ctx.store.profile_modified(n)).unwrap_or(false);
        self.prof.autosave = self.ctx.store.autosave();
        self.cur_state = self.ctx.store.read_state();
        self.prof.sel = self.prof.sel.min(self.prof.names.len().saturating_sub(1));
    }

    pub(crate) fn profile_unsaved(&self) -> bool {
        self.prof.active.is_some() && self.prof.modified && !self.prof.autosave
    }

    pub(crate) fn toggle_autosave(&mut self) {
        self.prof.autosave = !self.prof.autosave;
        self.ctx.store.set_autosave(self.prof.autosave);
        self.log.push(if self.prof.autosave {
            match &self.prof.active {
                Some(a) => format!("auto-save on: changes you apply are also saved into profile '{a}'"),
                None => "auto-save on (it takes effect once there is an active profile)".into(),
            }
        } else {
            "auto-save off: save changes yourself with [ Save ]".into()
        });
    }

    pub(crate) fn save_into_active(&mut self) {
        match self.prof.active.clone() {
            Some(a) => self.spawn(vec![vec!["profile".into(), "save".into(), a]]),
            None => self.start_naming(NameKind::SaveAs),
        }
    }

    pub(crate) fn start_naming(&mut self, kind: NameKind) {
        self.modal.naming = Some(match &kind {
            NameKind::Rename(old) => old.clone(),
            NameKind::SaveAs | NameKind::ImportPath => String::new(),
        });
        self.modal.naming_kind = kind;
    }

    pub(crate) fn rename_selected(&mut self) {
        if let Some(n) = self.prof.names.get(self.prof.sel).cloned() {
            self.start_naming(NameKind::Rename(n));
        }
    }

    /// Enter in the name prompt.
    pub(crate) fn finish_naming(&mut self) {
        let Some(name) = self.modal.naming.take() else { return };
        if name.is_empty() {
            return;
        }
        match self.modal.naming_kind.clone() {
            NameKind::SaveAs => {
                if self.prof.names.contains(&name) && self.prof.active.as_deref() != Some(name.as_str()) {
                    self.log.push(format!("error: a profile named '{name}' already exists"));
                } else {
                    self.spawn(vec![vec!["profile".into(), "save".into(), name]]);
                }
            }
            NameKind::ImportPath => {
                let path = clean_path(&name);
                self.spawn(vec![vec!["profile".into(), "import".into(), path]]);
            }
            NameKind::Rename(old) => {
                if old != name {
                    self.spawn(vec![vec!["profile".into(), "rename".into(), old, name]]);
                }
            }
        }
    }

    /// Clickable buttons on the profiles page: (label, enabled, id), laid out left to right with two spaces between.
    pub(crate) fn profile_buttons(&self) -> Vec<(String, bool, u8)> {
        let have = !self.prof.names.is_empty();
        let save = match &self.prof.active {
            Some(a) => text::profiles::save_into(a),
            None => text::profiles::SAVE.to_string(),
        };
        let del = if self.modal.confirm_delete { text::profiles::CONFIRM_DELETE } else { text::profiles::DELETE };
        [
            (text::profiles::LOAD.to_string(), have, 0),
            (save, true, 1),
            (text::profiles::SAVE_AS.to_string(), true, 2),
            (text::profiles::RENAME.to_string(), have, 3),
            (del.to_string(), have, 4),
        ]
        .into_iter()
        .map(|(label, enabled, id)| (tr(&label).into_owned(), enabled, id))
        .collect()
    }

    pub(crate) fn profile_button_click(&mut self, id: u8) {
        match id {
            0 => self.load_selected_profile(),
            1 => self.save_into_active(),
            2 => self.start_naming(NameKind::SaveAs),
            3 => self.rename_selected(),
            _ => self.delete_selected_profile(),
        }
    }

    pub(crate) fn load_selected_profile(&mut self) {
        if let Some(n) = self.prof.names.get(self.prof.sel).cloned() {
            self.spawn(vec![vec!["profile".into(), "load".into(), n]]);
        }
    }

    pub(crate) fn delete_selected_profile(&mut self) {
        if self.prof.names.is_empty() {
            return;
        }
        if self.modal.confirm_delete {
            let n = self.prof.names[self.prof.sel].clone();
            self.modal.confirm_delete = false;
            self.spawn(vec![vec!["profile".into(), "delete".into(), n]]);
        } else {
            self.modal.confirm_delete = true;
        }
    }
}

/// A path as typed or pasted: no surrounding quotes or spaces, `~/` expanded.
pub(crate) fn clean_path(raw: &str) -> String {
    let p = raw.trim().trim_matches(|c| c == '"' || c == '\'');
    let p = p.strip_prefix("file://").unwrap_or(p);
    match (p.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
        _ => p.to_string(),
    }
}

/// `summarize`, translated piece by piece (the pieces are joined with ", ").
pub(crate) fn summary(st: &State) -> String {
    summarize(st).split(", ").map(|p| tr_ctx("summary", p).into_owned()).collect::<Vec<_>>().join(", ")
}
