//! Turning rows into commands, running them in the background and applying the results.

use crate::tui::*;

/// Runs one command line (the same ones the CLI takes) and returns what it printed.
pub trait Runner: Send + Sync {
    fn run(&self, args: &[String]) -> Result<String, String>;
    /// Commands seen so far (only recording runners keep them; the unit tests read them).
    #[allow(dead_code)]
    fn log(&self) -> Vec<Vec<String>> {
        Vec::new()
    }
}

/// Runs commands inside this process.
pub(crate) struct InProcess(pub(crate) Context);

impl Runner for InProcess {
    fn run(&self, args: &[String]) -> Result<String, String> {
        let mut out = Vec::new();
        crate::cli::execute(args, &self.0, &mut out)?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}

impl App {
    /// Commands for the Apply of one group of rows on a page. Each command is the argument list after `m7ctl`.
    pub(crate) fn commands(&self, tab: TabId, group: u8, row_label: &str) -> Result<Vec<Vec<String>>, String> {
        let t = self.tabs.iter().find(|t| t.id == tab).ok_or("no such page")?;
        let get = |label: &str| t.rows.iter().find(|r| r.label.starts_with(label)).map(|r| r.value.clone());
        let s = |v: &str| v.to_string();
        let cmds = match tab {
            TabId::Dpi if group == 0 => {
                let n = match get("Stages in use") {
                    Some(Value::Num(n)) => n as usize,
                    _ => 1,
                };
                let stages: Vec<String> = (1..=n)
                    .map(|i| match get(&format!("Stage {i} DPI")) {
                        Some(Value::Num(v)) => v.to_string(),
                        _ => "800".into(),
                    })
                    .collect();
                let mut active = match get("Starting stage") {
                    Some(Value::Num(a)) => a.min(n as i64).max(1),
                    _ => 1,
                };
                // Writing the stages makes the mouse jump to the starting stage. Unless that row was edited,
                // keep the stage the mouse is on now.
                let untouched = t
                    .rows
                    .iter()
                    .position(|r| r.label.starts_with("Starting stage"))
                    .map(|i| t.applied.get(i).map(|a| a.0 == t.rows[i].value).unwrap_or(true))
                    .unwrap_or(true);
                if let (Some(live), true) = (self.live.live_stage, untouched) {
                    active = (live as i64).min(n as i64).max(1);
                }
                vec![vec![s("dpi"), stages.join(","), s("--active"), active.to_string()]]
            }
            TabId::Dpi => {
                let c = match get("Scroll-light colours") {
                    Some(Value::Text(c)) => c,
                    _ => String::new(),
                };
                vec![vec![s("dpi-colors"), c.trim().to_string()]]
            }
            TabId::Rate => match get("Polling rate") {
                Some(Value::Choice(i)) => vec![vec![s("rate"), RATES[i].to_string()]],
                _ => vec![],
            },
            TabId::Lights => {
                let mode = match get("Mode") {
                    Some(Value::Choice(i)) => MODES[i],
                    _ => "steady",
                };
                let colours = match get("Colours") {
                    Some(Value::Text(c)) => c.trim().to_string(),
                    _ => String::new(),
                };
                let bright = match get("Brightness") {
                    Some(Value::Num(b)) => b.to_string(),
                    _ => "3".into(),
                };
                let speed = match get("Cycle speed") {
                    Some(Value::Num(b)) => b.to_string(),
                    _ => "1".into(),
                };
                match mode {
                    "off" => vec![vec![s("light"), s("off")]],
                    "cycle" => vec![vec![s("light"), s("cycle"), colours, s("--brightness"), bright, s("--speed"), speed]],
                    m => vec![vec![s("light"), s(m), colours.split(',').next().unwrap_or("").trim().to_string(), s("--brightness"), bright]],
                }
            }
            TabId::Buttons => {
                let label = if row_label.is_empty() { BUTTONS[group as usize] } else { row_label.trim_end_matches(" button") };
                let f = match t.rows.iter().find(|r| r.label == format!("{label} button")).map(|r| (r.value.clone(), r.extra.trim().to_string())) {
                    Some((Value::Choice(i), extra)) => {
                        if i < FUNCS.len() {
                            FUNCS[i].to_string()
                        } else {
                            extra
                        }
                    }
                    _ => "left".to_string(),
                };
                if f.is_empty() {
                    return Err("type the custom function first (for example key:ctrl+c)".into());
                }
                vec![vec![s("button"), s(label), f]]
            }
            TabId::Macros => {
                let slot = match get("1. Slot") {
                    Some(Value::Num(n)) => n,
                    _ => 1,
                };
                match group {
                    0 => {
                        let steps = match get("2. Steps") {
                            Some(Value::Text(t)) => t.trim().to_string(),
                            _ => String::new(),
                        };
                        if steps.is_empty() {
                            return Err("type the macro steps first (for example tap:ctrl+c,wait:100)".into());
                        }
                        vec![vec![s("macro"), slot.to_string(), steps]]
                    }
                    1 => vec![vec![s("macro"), slot.to_string(), s("clear")]],
                    _ => {
                        let b = match get("4. Button") {
                            Some(Value::Choice(i)) => BUTTONS[i],
                            _ => "forward",
                        };
                        let times = match get("Times") {
                            Some(Value::Num(n)) => n,
                            _ => 1,
                        };
                        let f = match get("Repeat") {
                            Some(Value::Choice(0)) => format!("macro:{slot},0"),
                            Some(Value::Choice(1)) => format!("macro:{slot},1"),
                            _ => format!("macro:{slot},2,{times}"),
                        };
                        vec![vec![s("button"), s(b), f]]
                    }
                }
            }
            TabId::Power => match group {
                0 => {
                    let m = match get("Power-down") {
                        Some(Value::Num(m)) => m,
                        _ => 6,
                    };
                    let mut c = vec![s("power"), m.to_string()];
                    if get("Lights off while") == Some(Value::Bool(true)) {
                        c.push(s("lights-off-moving"));
                    }
                    vec![c]
                }
                1 => vec![vec![s("motion-wake"), s(if get("Motion wake") == Some(Value::Bool(true)) { "on" } else { "off" })]],
                _ => match get("Lift-off") {
                    Some(Value::Choice(i)) => vec![vec![s("lod"), s(["1", "2"][i])]],
                    _ => vec![],
                },
            },
            _ => vec![],
        };
        Ok(cmds)
    }

    pub(crate) fn spawn(&mut self, cmds: Vec<Vec<String>>) {
        self.spawn_marked(cmds, None);
    }

    /// Run commands; when `mark` is set and every command succeeds, that group of rows counts as applied.
    pub(crate) fn spawn_marked(&mut self, cmds: Vec<Vec<String>>, mark: Option<(usize, u8)>) {
        if cmds.iter().any(|c| c.first().map(String::as_str) == Some("profile") && c.get(1).map(String::as_str) == Some("load")) {
            self.bg.reload_after = true;
        }
        if self.bg.busy {
            self.log.push("still busy with the previous command, wait a moment".into());
            return;
        }
        let autosave_to = if mark.is_some() && self.prof.autosave { self.prof.active.clone() } else { None };
        self.bg.busy = true;
        let (tx, runner) = (self.bg.tx.clone(), self.bg.runner.clone());
        self.run_job(move || {
            let mut ok = true;
            for args in cmds {
                match runner.run(&args) {
                    Ok(o) => {
                        if o.contains("NOT applied") || o.contains("NOT stored") || o.contains("nothing acknowledged") {
                            ok = false;
                        }
                        for l in o.lines() {
                            // keep the log short: drop profile-load step lines and the long acknowledgement wording
                            if l.starts_with('[') && l.contains('/') && l.chars().take(8).any(|c| c == ']') {
                                continue;
                            }
                            let l = l.replace(" (the mouse acknowledged it)", "").replace(" and is now the active profile", "");
                            let l = shorten_macro_line(&l);
                            let _ = tx.send(Msg::Line(l));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Line(format!("error: {e}")));
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                if let Some((t, g)) = mark {
                    let _ = tx.send(Msg::Applied(t, g));
                }
                if let Some(n) = autosave_to {
                    let line = match runner.run(&["profile".to_string(), "save".into(), n.clone()]) {
                        Ok(_) => format!("saved into profile '{n}' (auto-save)"),
                        Err(e) => format!("error: could not save into profile '{n}': {e}"),
                    };
                    let _ = tx.send(Msg::Line(line));
                }
            }
            let _ = tx.send(Msg::Done);
        });
    }

    /// Run background work on a thread (tests run it inline so they stay deterministic).
    pub(crate) fn run_job(&self, job: impl FnOnce() + Send + 'static) {
        if self.bg.inline {
            job();
        } else {
            std::thread::spawn(job);
        }
    }

    pub(crate) fn drain(&mut self) {
        while let Ok(m) = self.bg.rx.try_recv() {
            match m {
                Msg::Line(l) => self.log.push(l),
                Msg::Applied(t, g) => {
                    if let Some(tab) = self.tabs.get_mut(t) {
                        for i in 0..tab.rows.len() {
                            if tab.rows[i].group == g {
                                tab.applied[i] = (tab.rows[i].value.clone(), tab.rows[i].extra.clone());
                            }
                        }
                    }
                }
                Msg::Link(link) => {
                    let first = self.live.conn.is_none();
                    if !first {
                        self.log.push(match link.as_str() {
                            "" => "mouse disconnected".to_string(),
                            l => format!("mouse connected ({l})"),
                        });
                    }
                    self.live.conn = Some(link.clone());
                    self.live.live_stage = None;
                    if link.is_empty() {
                        self.live.status = StatusInfo::default();
                    } else if !first {
                        // read battery and link again as soon as the worker is free
                        self.live.last_status = std::time::Instant::now().checked_sub(Duration::from_secs(3600)).unwrap_or(self.live.last_status);
                    }
                }
                Msg::Dpi(stage, dpi) => {
                    self.live.live_stage = Some(stage);
                    self.log.push(format!("DPI stage {stage}: {dpi}"));
                }
                Msg::Status(s) => {
                    self.live.status = s;
                    self.live.last_status = std::time::Instant::now();
                }
                Msg::Done => {
                    self.bg.busy = false;
                    self.refresh_profiles();
                    if self.bg.reload_after {
                        // a profile was loaded: show the values it wrote
                        self.bg.reload_after = false;
                        let fresh = App::new(self.ctx.clone(), self.bg.runner.clone(), self.bg.inline);
                        self.tabs = fresh.tabs;
                    }
                }
            }
        }
        if self.log.len() > 200 {
            self.log.drain(..self.log.len() - 200);
        }
    }

    /// Change the selected row's value: Left/Right, PgUp/PgDn, mouse arrows and wheel.
    pub(crate) fn adjust(&mut self, dir: i64, big: bool) {
        let t = &mut self.tabs[self.cur];
        if t.rows.is_empty() {
            return;
        }
        let row = &mut t.rows[t.sel];
        match &mut row.value {
            Value::Choice(i) => {
                let n = row.opts.len() as i64;
                *i = (*i as i64 + dir).rem_euclid(n) as usize;
            }
            Value::Num(v) => {
                let (min, max, step) = row.range;
                *v = (*v + dir * step * if big { 10 } else { 1 }).clamp(min, max);
            }
            Value::Bool(b) => *b = !*b,
            _ => {}
        }
        self.after_change();
    }

    /// What follows a changed value on some pages (custom function typing, stage rows, macro slot contents).
    fn after_change(&mut self) {
        let r = &self.tabs[self.cur].rows[self.tabs[self.cur].sel];
        if self.tabs[self.cur].id == TabId::Buttons && is_custom_choice(r) && r.extra.trim().is_empty() {
            self.modal.editing_custom = true;
        }
        if self.tabs[self.cur].id == TabId::Dpi && self.tabs[self.cur].rows[self.tabs[self.cur].sel].label == "Stages in use" {
            self.rebuild_dpi();
        }
        if self.tabs[self.cur].id == TabId::Macros && self.tabs[self.cur].rows[self.tabs[self.cur].sel].label == "1. Slot" {
            self.load_macro_slot();
        }
    }

    /// Enter / double click / right click on the selected row: apply it, or start editing a text row.
    pub(crate) fn activate(&mut self) {
        let tab = self.tab().id;
        if tab == TabId::Status {
            self.refresh_status();
            return;
        }
        let t = self.tab();
        if t.rows.is_empty() {
            return;
        }
        let row = t.rows[t.sel].clone();
        if is_color_row(&row) {
            self.open_picker();
            return;
        }
        if let Value::Text(_) = row.value {
            self.editing = true;
            return;
        }
        if tab == TabId::Buttons {
            self.open_chooser();
            return;
        }
        self.apply_selected();
    }

    /// Apply the group of rows the selected row belongs to (marks them as applied when it succeeds).
    pub(crate) fn apply_selected(&mut self) {
        let tab = self.tab().id;
        if matches!(tab, TabId::Status | TabId::Profiles) || self.tab().rows.is_empty() {
            return;
        }
        let row = self.tab().rows[self.tab().sel].clone();
        if tab == TabId::Macros && matches!(row.group, 3..=5) {
            match row.group {
                3 => self.modal.recording = Some(Recorder::default()),
                4 => self.open_add_step(),
                _ => self.undo_last_step(),
            }
            return;
        }
        if tab == TabId::Macros && row.group == 1 {
            // clearing empties the steps; that is not an unapplied change
            let t = &mut self.tabs[self.cur];
            t.rows[1].value = Value::Text(String::new());
            t.applied[1] = (Value::Text(String::new()), String::new());
        }
        let label = if tab == TabId::Buttons { row.label.clone() } else { String::new() };
        match self.commands(tab, row.group, &label) {
            Ok(c) if !c.is_empty() => {
                let mark = (self.cur, row.group);
                self.spawn_marked(c, Some(mark));
            }
            Ok(_) => {}
            Err(e) => self.log.push(format!("error: {e}")),
        }
    }

    /// Put every changed row of this page back to what was last applied.
    pub(crate) fn revert_page(&mut self) {
        let t = &mut self.tabs[self.cur];
        let mut n = 0;
        for i in 0..t.rows.len() {
            if t.dirty_row(i) {
                t.rows[i].value = t.applied[i].0.clone();
                t.rows[i].extra = t.applied[i].1.clone();
                n += 1;
            }
        }
        self.log.push(if n == 0 { "nothing to undo on this page".into() } else { format!("undid {n} unapplied change(s) on this page") });
    }
}

impl App {
    /// The selected row takes a typed number (the `e` key): a numeric row, or a choice made of numbers (polling rate, lift-off).
    pub(crate) fn selected_numeric(&self) -> bool {
        let t = self.tab();
        if matches!(t.id, TabId::Status | TabId::Profiles) {
            return false;
        }
        match t.rows.get(t.sel) {
            Some(r) => match r.value {
                Value::Num(_) => true,
                Value::Choice(_) => !r.opts.is_empty() && r.opts.iter().all(|o| o.trim().parse::<i64>().is_ok()),
                _ => false,
            },
            None => false,
        }
    }

    /// The selected row can be typed into with `e`: a number, a colour list or a custom button function.
    pub(crate) fn selected_editable(&self) -> bool {
        let t = self.tab();
        self.selected_numeric() || t.rows.get(t.sel).is_some_and(|r| is_color_row(r) || (t.id == TabId::Buttons && is_custom_choice(r)))
    }

    pub(crate) fn typing_key(&mut self, k: KeyEvent) {
        let Some(buf) = self.typing.as_mut() else { return };
        match k.code {
            KeyCode::Char(c) if c.is_ascii_digit() && !k.modifiers.contains(KeyModifiers::CONTROL) => {
                if buf.len() < 6 {
                    buf.push(c);
                }
            }
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Enter => self.commit_typing(),
            KeyCode::Esc => self.typing = None,
            // typing a value and moving on: 800 Down 1600 Down ...
            KeyCode::Up | KeyCode::Down => {
                self.commit_typing();
                self.key(k);
            }
            _ => {}
        }
    }

    /// Store the typed number: snapped to the row's range and step, or to one of its options.
    pub(crate) fn commit_typing(&mut self) {
        let Some(buf) = self.typing.take() else { return };
        let Ok(typed) = buf.parse::<i64>() else { return };
        let (cur, sel) = (self.cur, self.tabs[self.cur].sel);
        let row = &mut self.tabs[cur].rows[sel];
        match &mut row.value {
            Value::Num(n) => {
                let (min, max, step) = row.range;
                let v = typed.clamp(min, max);
                *n = (min + ((v - min) as f64 / step as f64).round() as i64 * step).clamp(min, max);
            }
            Value::Choice(i) => match row.opts.iter().position(|o| o.trim().parse::<i64>().ok() == Some(typed)) {
                Some(p) => *i = p,
                None => {
                    let options = row.opts.join(", ");
                    self.log.push(format!("{typed} is not an option ({options})"));
                    return;
                }
            },
            _ => return,
        }
        self.after_change();
    }
}
