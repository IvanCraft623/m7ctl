//! Macros page.

use crate::tui::*;

/// Recorded steps of a macro slot ("" when none).
pub(crate) fn macro_steps(state: &[(String, Vec<String>)], slot: i64) -> String {
    state.iter().find(|(k, _)| *k == format!("macro:{slot}")).and_then(|(_, v)| v.get(1).cloned()).unwrap_or_default()
}

/// The macros page, in the order you use it: 1 pick a slot, 2 build the steps, 3 save, 4 give it to a button.
pub(crate) fn macro_rows(slot: i64, steps: &str) -> Vec<Row> {
    vec![
        num("1. Slot", slot, 1, 15, 1, 0),
        text("2. Steps", steps, 0),
        action("Record keys", 3),
        action("Add a step...", 4),
        action("Undo last step", 5),
        action("3. Save to the mouse", 0),
        choice("4. Button", BUTTONS, 3, 2),
        choice("Repeat", &["loop", "while held", "times"], 2, 2),
        num("Times", 1, 1, 255, 1, 2),
        action("Assign to the button", 2),
        action("Clear this slot", 1),
    ]
}

/// One macro step in plain words.
pub(crate) fn describe_step(step: &str) -> String {
    tr(&describe_step_en(step)).into_owned()
}

fn describe_step_en(step: &str) -> String {
    match step.split_once(':') {
        Some(("tap", k)) => format!("press {k}"),
        Some(("down", k)) => format!("hold {k}"),
        Some(("up", k)) => format!("release {k}"),
        Some(("type", t)) => format!("type \"{t}\""),
        Some(("click", b)) => format!("click {b}"),
        Some(("wait", ms)) => format!("wait {ms} ms"),
        _ => step.to_string(),
    }
}

pub(crate) fn describe_steps(spec: &str) -> String {
    let parts: Vec<String> = spec.split(',').map(str::trim).filter(|x| !x.is_empty()).map(describe_step).collect();
    if parts.is_empty() {
        return tr("(empty: record keys or add a step)").into_owned();
    }
    parts.join(", ")
}

/// Cut `s` to at most `max` characters, ending with `~` when something was dropped.
pub(crate) fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "~"
    }
}

/// "stored", "stored - on the forward button", or "empty" for a macro slot.
pub(crate) fn slot_status(state: &[(String, Vec<String>)], slot: i64) -> String {
    if !state.iter().any(|(k, _)| *k == format!("macro:{slot}")) {
        return "empty".into();
    }
    let prefix = format!("macro:{slot},");
    let on: Vec<String> = state
        .iter()
        .filter(|(k, v)| k.starts_with("button:") && v.get(1).map(|f| f.starts_with(&prefix) || *f == format!("macro:{slot}")).unwrap_or(false))
        .map(|(k, _)| k.trim_start_matches("button:").to_string())
        .collect();
    if on.is_empty() {
        "stored".into()
    } else {
        format!("stored - on the {} button", on.join(" and "))
    }
}

/// Text for the Steps row's tag: how much of the mouse's 125-event limit it uses.
pub(crate) fn steps_tag(spec: &str) -> String {
    if spec.trim().is_empty() {
        return String::new();
    }
    match crate::protocol::parse_macro(spec) {
        Ok(ev) => format!("{}/{} events", ev.len(), crate::protocol::MACRO_MAX_EVENTS),
        Err(e) => format!("invalid: {e}"),
    }
}

/// `macro 3: 12 events uploaded; assign it with ...` -> `macro 3 stored (12 events)`
pub(crate) fn shorten_macro_line(l: &str) -> String {
    if let (Some(rest), true) = (l.strip_prefix("macro "), l.contains("events uploaded")) {
        let slot = rest.split(':').next().unwrap_or("");
        let n = rest.split(": ").nth(1).and_then(|x| x.split(' ').next()).unwrap_or("");
        return format!("macro {slot} stored ({n} events)");
    }
    if let (Some(rest), true) = (l.strip_prefix("macro "), l.contains("cleared")) {
        return format!("macro {} cleared", rest.split(':').next().unwrap_or(""));
    }
    l.to_string()
}

impl App {
    /// A different macro slot was chosen: show what the record says it holds (browsing is not a change).
    pub(crate) fn load_macro_slot(&mut self) {
        let t = &mut self.tabs[self.cur];
        let slot = match t.rows[0].value {
            Value::Num(n) => n,
            _ => 1,
        };
        t.rows[1].value = Value::Text(macro_steps(&self.cur_state, slot));
        t.applied[0] = (t.rows[0].value.clone(), String::new());
        t.applied[1] = (t.rows[1].value.clone(), String::new());
    }

    pub(crate) fn macros_tab(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == TabId::Macros)
    }

    pub(crate) fn steps_now(&self) -> String {
        match self.macros_tab().map(|i| &self.tabs[i].rows[1].value) {
            Some(Value::Text(t)) => t.trim().to_string(),
            _ => String::new(),
        }
    }

    /// Append steps (a comma separated list) to the macro being built, if the mouse could store the result.
    pub(crate) fn append_steps(&mut self, add: &str) -> bool {
        let Some(ti) = self.macros_tab() else { return false };
        let cur = self.steps_now();
        let new = if cur.is_empty() { add.to_string() } else { format!("{cur},{add}") };
        if let Err(e) = crate::protocol::parse_macro(&new) {
            self.log.push(format!("error: {e}"));
            return false;
        }
        self.tabs[ti].rows[1].value = Value::Text(new);
        true
    }

    pub(crate) fn undo_last_step(&mut self) {
        let Some(ti) = self.macros_tab() else { return };
        let cur = self.steps_now();
        match cur.rfind(',') {
            Some(i) => self.tabs[ti].rows[1].value = Value::Text(cur[..i].to_string()),
            None => self.tabs[ti].rows[1].value = Value::Text(String::new()),
        }
    }

    pub(crate) fn open_add_step(&mut self) {
        let items: Vec<String> = [
            "Press a key or combo...",
            "Type text...",
            "Click left button",
            "Click right button",
            "Click middle button",
            "Click back button",
            "Click forward button",
            "Hold a key down...",
            "Release a key...",
            "Wait...",
        ]
        .iter()
        .map(|x| x.to_string())
        .collect();
        self.modal.chooser =
            Some(Chooser { row: self.tabs[self.cur].sel, filter: String::new(), sel: 0, items: Some(("add a step".into(), items)), lang: false });
    }

    pub(crate) fn add_step_pick(&mut self, label: &str) {
        let prompt = |kind, text: &str| Some(StepPrompt { kind, text: text.into() });
        if let Some(b) = label.strip_prefix("Click ").and_then(|x| x.split(' ').next()) {
            let step = format!("click:{b}");
            self.append_steps(&step);
            return;
        }
        self.modal.step_prompt = match label {
            "Press a key or combo..." => prompt(StepKind::Tap, ""),
            "Type text..." => prompt(StepKind::Type, ""),
            "Hold a key down..." => prompt(StepKind::Down, ""),
            "Release a key..." => prompt(StepKind::Up, ""),
            _ => prompt(StepKind::Wait, "100"),
        };
    }

    pub(crate) fn finish_step_prompt(&mut self) {
        let Some(p) = self.modal.step_prompt.take() else { return };
        let text = p.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let step = match p.kind {
            StepKind::Tap => format!("tap:{}", text.to_lowercase()),
            StepKind::Down => format!("down:{}", text.to_lowercase()),
            StepKind::Up => format!("up:{}", text.to_lowercase()),
            StepKind::Type => format!("type:{text}"),
            StepKind::Wait => format!("wait:{text}"),
        };
        // a pause only makes sense after another step; everything else can be checked on its own
        if p.kind != StepKind::Wait {
            if let Err(e) = crate::protocol::parse_macro(&step) {
                self.log.push(format!("error: {e}"));
                return;
            }
        }
        self.append_steps(&step);
    }

    /// F2 while recording: the keys pressed become steps.
    pub(crate) fn finish_recording(&mut self) {
        let Some(r) = self.modal.recording.take() else { return };
        let ignored = r.ignored;
        let spec = r.finish();
        if spec.is_empty() {
            self.log.push("nothing recorded".into());
            return;
        }
        let n = spec.split(',').count();
        if self.append_steps(&spec) {
            self.log.push(format!(
                "recorded {n} step{}{}",
                if n == 1 { "" } else { "s" },
                if ignored > 0 { format!(" ({ignored} key(s) ignored: the mouse cannot store them)") } else { String::new() }
            ));
        }
    }

    pub(crate) fn recording_key(&mut self, k: KeyEvent) {
        if k.kind != KeyEventKind::Press {
            return;
        }
        match k.code {
            KeyCode::F(2) => self.finish_recording(),
            KeyCode::F(3) => {
                self.modal.recording = None;
                self.log.push("recording cancelled".into());
            }
            _ => {
                if let Some(r) = self.modal.recording.as_mut() {
                    r.key(&k, std::time::Instant::now());
                }
            }
        }
    }
}
