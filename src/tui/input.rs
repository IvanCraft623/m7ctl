//! Keyboard and mouse handling.

use crate::tui::*;

impl App {
    pub(crate) fn mouse(&mut self, m: MouseEvent) {
        if self.modal.recording.is_some() {
            self.recording_mouse(m);
            return;
        }
        if self.modal.step_prompt.is_some() || self.modal.naming.is_some() {
            self.prompt_mouse(m);
            return;
        }
        if self.modal.chooser.is_some() {
            self.chooser_mouse(m);
            return;
        }
        if self.modal.quit_prompt.is_some() {
            self.quit_mouse(m);
            return;
        }
        if self.modal.help {
            if matches!(m.kind, MouseEventKind::Down(_)) {
                self.modal.help = false;
            }
            return;
        }
        if self.modal.picker.is_some() {
            self.picker_mouse(m);
            return;
        }
        if self.typing.is_some() && matches!(m.kind, MouseEventKind::Down(_)) {
            self.commit_typing();
        }
        let [tabs_a, body_a, _] = areas(self.area);
        let (col, row) = (m.column, m.row);
        // page titles
        if row >= tabs_a.y && row < tabs_a.y + tabs_a.height {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                for (i, (x0, x1)) in tab_ranges().into_iter().enumerate() {
                    if col >= tabs_a.x + 1 + x0 && col < tabs_a.x + 1 + x1 {
                        self.cur = i;
                        self.editing = false;
                        self.modal.editing_custom = false;
                        self.modal.confirm_delete = false;
                    }
                }
            }
            return;
        }
        let foot = footer_rect(self.area);
        if m.row == foot.y && m.column >= foot.x {
            if let MouseEventKind::Down(MouseButton::Left) = m.kind {
                self.footer_click(m.column - foot.x);
            }
            return;
        }
        let (x0, y0) = (body_a.x + 1, body_a.y + 1);
        if col < x0 || row < y0 || row + 1 >= body_a.y + body_a.height {
            return;
        }
        let line = (row - y0) as usize;
        let off = col - x0;
        let is_profiles = self.tab().id == TabId::Profiles;
        let kind = m.kind;
        if is_profiles {
            // lines: 0 active, 1 auto-save, 3 buttons, 5 header, 6.. profiles
            match kind {
                MouseEventKind::ScrollUp => self.prof.sel = self.prof.sel.saturating_sub(1),
                MouseEventKind::ScrollDown => self.prof.sel = (self.prof.sel + 1).min(self.prof.names.len().saturating_sub(1)),
                MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => match line {
                    1 => self.toggle_autosave(),
                    3 => {
                        let mut x = 0u16;
                        for (label, enabled, id) in self.profile_buttons() {
                            let w = label.chars().count() as u16;
                            if off >= x && off < x + w {
                                if enabled {
                                    self.profile_button_click(id);
                                }
                                break;
                            }
                            x += w + 2;
                        }
                    }
                    l if l >= P_FIRST && l - P_FIRST < self.prof.names.len() => {
                        self.prof.sel = l - P_FIRST;
                        self.modal.confirm_delete = false;
                        if self.double_click(line) || kind == MouseEventKind::Down(MouseButton::Right) {
                            self.load_selected_profile();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
            return;
        }
        let n = self.tab().rows.len();
        match kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let dir: i64 = if kind == MouseEventKind::ScrollUp { 1 } else { -1 };
                if line < n && off >= VALUE_X {
                    self.tabs[self.cur].sel = line;
                    self.adjust(dir, false);
                } else {
                    let t = &mut self.tabs[self.cur];
                    t.sel = if dir > 0 { t.sel.saturating_sub(1) } else { (t.sel + 1).min(n.saturating_sub(1)) };
                }
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
                if line >= n {
                    return;
                }
                self.tabs[self.cur].sel = line;
                self.editing = false;
                self.modal.editing_custom = false;
                let value = self.tabs[self.cur].rows[line].value.clone();
                let right_click = kind == MouseEventKind::Down(MouseButton::Right);
                match value {
                    Value::Action => self.activate(),
                    Value::Text(_) => {
                        if self.tabs[self.cur].rows[line].label.contains("olour") {
                            self.open_picker();
                        } else if right_click {
                            self.activate()
                        } else {
                            self.editing = true
                        }
                    }
                    _ if right_click => {
                        if self.tab().id == TabId::Buttons {
                            self.apply_selected()
                        } else {
                            self.activate()
                        }
                    }
                    _ => {
                        let stage =
                            self.tabs[self.cur].rows[line].label.strip_prefix("Stage ").and_then(|x| x.split(' ').next()).and_then(|x| x.parse::<usize>().ok());
                        if let (Some(k), true) = (stage, self.tab().id == TabId::Dpi && (VALUE_X + 14..VALUE_X + 18).contains(&off)) {
                            self.open_picker_at(k - 1);
                        } else if off >= VALUE_X {
                            let shown = value_width(&self.tabs[self.cur].rows[line]);
                            let half = VALUE_X + shown / 2;
                            if off < VALUE_X + shown {
                                if self.tab().id == TabId::Buttons {
                                    // clicking the function opens the list, like Enter
                                    self.open_chooser();
                                } else {
                                    self.adjust(if off < half { -1 } else { 1 }, false);
                                }
                            }
                        } else if self.double_click(line) {
                            self.activate();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn double_click(&mut self, line: usize) -> bool {
        let now = std::time::Instant::now();
        let hit = matches!(self.last_click, Some((t, tab, l)) if tab == self.cur && l == line && now.duration_since(t) < Duration::from_millis(500));
        self.last_click = if hit { None } else { Some((now, self.cur, line)) };
        hit
    }

    pub(crate) fn key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press {
            return;
        }
        if self.modal.recording.is_some() {
            self.recording_key(k);
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.modal.help {
            self.modal.help = false;
            return;
        }
        if self.modal.quit_prompt.is_some() {
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.quit_choice(0),
                KeyCode::Char('s') | KeyCode::Char('S') if self.profile_unsaved() => self.quit_choice(1),
                _ => self.quit_choice(2),
            }
            return;
        }
        if self.modal.chooser.is_some() {
            self.chooser_key(k);
            return;
        }
        if self.modal.picker.is_some() {
            self.picker_key(k);
            return;
        }
        // naming a profile to save, or typing the path of a file to import (any character)
        let importing = self.modal.naming_kind == NameKind::ImportPath;
        if let Some(name) = self.modal.naming.as_mut() {
            match k.code {
                KeyCode::Esc => self.modal.naming = None,
                KeyCode::Enter => self.finish_naming(),
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Char(c) if importing || c.is_ascii_alphanumeric() || c == '-' || c == '_' => name.push(c),
                _ => {}
            }
            return;
        }
        // answering a question on the macros page
        if self.modal.step_prompt.is_some() {
            match k.code {
                KeyCode::Esc => self.modal.step_prompt = None,
                KeyCode::Enter => self.finish_step_prompt(),
                KeyCode::Backspace => {
                    self.modal.step_prompt.as_mut().unwrap().text.pop();
                }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => self.modal.step_prompt.as_mut().unwrap().text.push(c),
                _ => {}
            }
            return;
        }
        // typing a custom button function
        if self.modal.editing_custom {
            let (cur, sel) = (self.cur, self.tabs[self.cur].sel);
            let mut stop = false;
            {
                let extra = &mut self.tabs[cur].rows[sel].extra;
                match k.code {
                    KeyCode::Esc | KeyCode::Enter => stop = true,
                    KeyCode::Backspace => {
                        extra.pop();
                    }
                    KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => extra.clear(),
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => extra.push(c),
                    _ => {}
                }
            }
            if stop {
                self.modal.editing_custom = false;
            }
            return;
        }
        // editing a text row
        if self.editing {
            let cur = self.cur;
            let sel = self.tabs[cur].sel;
            if let Value::Text(t) = &mut self.tabs[cur].rows[sel].value {
                match k.code {
                    KeyCode::Esc | KeyCode::Enter => self.editing = false,
                    KeyCode::Backspace => {
                        t.pop();
                    }
                    KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => t.clear(),
                    KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => t.push(c),
                    _ => {}
                }
            } else {
                self.editing = false;
            }
            return;
        }
        if self.typing.is_some() {
            self.typing_key(k);
            return;
        }
        let is_profiles = self.tab().id == TabId::Profiles;
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.modal.confirm_delete {
                    self.modal.confirm_delete = false;
                } else {
                    self.request_quit();
                }
            }
            KeyCode::Char('?') | KeyCode::F(1) => self.modal.help = true,
            KeyCode::Home => {
                if is_profiles {
                    self.prof.sel = 0
                } else {
                    self.tabs[self.cur].sel = 0
                }
            }
            KeyCode::End => {
                if is_profiles {
                    self.prof.sel = self.prof.names.len().saturating_sub(1)
                } else {
                    let t = &mut self.tabs[self.cur];
                    t.sel = t.rows.len().saturating_sub(1)
                }
            }
            KeyCode::Tab => self.cur = (self.cur + 1) % self.tabs.len(),
            KeyCode::BackTab => self.cur = (self.cur + self.tabs.len() - 1) % self.tabs.len(),
            KeyCode::Char('e') if self.selected_numeric() => self.typing = Some(String::new()),
            KeyCode::Char(c @ '1'..='8') => self.cur = (c as usize - '1' as usize).min(self.tabs.len() - 1),
            KeyCode::Char('r') if self.tab().id == TabId::Status => self.refresh_status(),
            KeyCode::Char('e') if !is_profiles && self.tab().rows.get(self.tab().sel).map(is_color_row).unwrap_or(false) => self.editing = true,
            KeyCode::Up => {
                if is_profiles {
                    self.prof.sel = self.prof.sel.saturating_sub(1);
                } else {
                    let t = &mut self.tabs[self.cur];
                    t.sel = t.sel.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if is_profiles {
                    self.prof.sel = (self.prof.sel + 1).min(self.prof.names.len().saturating_sub(1));
                } else {
                    let t = &mut self.tabs[self.cur];
                    t.sel = (t.sel + 1).min(t.rows.len().saturating_sub(1));
                }
            }
            KeyCode::Left | KeyCode::Right | KeyCode::PageUp | KeyCode::PageDown if !is_profiles => {
                let dir: i64 = if matches!(k.code, KeyCode::Left | KeyCode::PageDown) { -1 } else { 1 };
                self.adjust(dir, matches!(k.code, KeyCode::PageUp | KeyCode::PageDown));
            }
            KeyCode::Enter if !is_profiles => self.activate(),
            KeyCode::Enter | KeyCode::Char('l') => self.load_selected_profile(),
            KeyCode::Char('s') if is_profiles => self.save_into_active(),
            KeyCode::Char('n') if is_profiles => self.start_naming(NameKind::SaveAs),
            KeyCode::Char('i') if is_profiles => self.start_naming(NameKind::ImportPath),
            KeyCode::Char('r') if is_profiles => self.rename_selected(),
            KeyCode::Char('d') if is_profiles => self.delete_selected_profile(),
            KeyCode::Char('A') if is_profiles => self.toggle_autosave(),
            KeyCode::Char('p') => self.toggle_popup(),
            KeyCode::Char('L') => self.open_language_chooser(),
            KeyCode::Char('a') if !is_profiles => self.apply_selected(),
            KeyCode::Char('u') if !is_profiles => self.revert_page(),
            KeyCode::Char('e')
                if !is_profiles && self.tab().id == TabId::Buttons && self.tab().rows.get(self.tab().sel).map(is_custom_choice).unwrap_or(false) =>
            {
                self.modal.editing_custom = true
            }
            _ => {}
        }
    }
}
