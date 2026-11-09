//! Key recorder and the "add a step" prompt of the Macros page.

use crate::tui::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum StepKind {
    Tap,
    Type,
    Down,
    Up,
    Wait,
}

/// A one-line question shown on the macros page while a step is being added.
#[derive(Clone, Debug)]
pub(crate) struct StepPrompt {
    pub(crate) kind: StepKind,
    pub(crate) text: String,
}

impl StepPrompt {
    pub(crate) fn label(&self) -> &'static str {
        match self.kind {
            StepKind::Tap => "Key or combo to press (e.g. ctrl+c, enter, f5)",
            StepKind::Type => "Text to type",
            StepKind::Down => "Key to hold down",
            StepKind::Up => "Key to release",
            StepKind::Wait => "Pause in milliseconds",
        }
    }
}

/// Keys pressed while recording become macro steps; quick typing is merged into one `type:` step
/// and pauses between keys become `wait:` steps.
#[derive(Default, Debug)]
pub(crate) struct Recorder {
    pub(crate) steps: Vec<String>,
    pub(crate) text: String,
    pub(crate) last: Option<std::time::Instant>,
    pub(crate) ignored: usize,
}

pub(crate) enum RecStep {
    Text(char),
    Tap(String),
}

/// Name of a printable key as the macro syntax spells it.
pub(crate) fn char_key_name(c: char) -> Option<String> {
    Some(match c {
        ' ' => "space".into(),
        '.' => "period".into(),
        '-' => "minus".into(),
        '/' => "slash".into(),
        ';' => "semicolon".into(),
        '=' => "equal".into(),
        '\'' => "quote".into(),
        ',' => "comma".into(),
        c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase().to_string(),
        _ => return None,
    })
}

pub(crate) fn rec_step(k: &KeyEvent) -> Option<RecStep> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    let shift = k.modifiers.contains(KeyModifiers::SHIFT);
    let combo = |name: &str| {
        let mut v = vec![];
        if ctrl {
            v.push("ctrl");
        }
        if alt {
            v.push("alt");
        }
        if shift {
            v.push("shift");
        }
        v.push(name);
        Some(RecStep::Tap(v.join("+")))
    };
    let step = match k.code {
        KeyCode::Char(c) if ctrl || alt => combo(&char_key_name(c)?),
        KeyCode::Char(c) if c.is_ascii_alphanumeric() || " .-/;='".contains(c) => Some(RecStep::Text(c)),
        KeyCode::Char(c) => Some(RecStep::Tap(char_key_name(c)?)),
        KeyCode::Enter => combo("enter"),
        KeyCode::Tab => combo("tab"),
        KeyCode::BackTab => Some(RecStep::Tap("shift+tab".into())),
        KeyCode::Backspace => combo("backspace"),
        KeyCode::Esc => combo("esc"),
        KeyCode::Delete => combo("delete"),
        KeyCode::Insert => combo("insert"),
        KeyCode::Home => combo("home"),
        KeyCode::End => combo("end"),
        KeyCode::PageUp => combo("pageup"),
        KeyCode::PageDown => combo("pagedown"),
        KeyCode::Left => combo("left"),
        KeyCode::Right => combo("right"),
        KeyCode::Up => combo("up"),
        KeyCode::Down => combo("down"),
        KeyCode::F(n @ 1..=12) => combo(&format!("f{n}")),
        _ => None,
    }?;
    // only keep what the mouse can store
    let ok = match &step {
        RecStep::Text(c) => crate::protocol::parse_macro(&format!("type:{c}")).is_ok(),
        RecStep::Tap(t) => crate::protocol::parse_macro(&format!("tap:{t}")).is_ok(),
    };
    if ok {
        Some(step)
    } else {
        None
    }
}

impl Recorder {
    pub(crate) fn flush_text(&mut self) {
        if !self.text.is_empty() {
            let t = std::mem::take(&mut self.text);
            self.steps.push(format!("type:{t}"));
        }
    }

    pub(crate) fn key(&mut self, k: &KeyEvent, now: std::time::Instant) {
        let Some(step) = rec_step(k) else {
            self.ignored += 1;
            return;
        };
        if let Some(last) = self.last {
            let gap = now.duration_since(last).as_millis();
            if gap >= 400 {
                self.flush_text();
                self.steps.push(format!("wait:{}", (gap.min(4000) + 5) / 10 * 10));
            }
        }
        self.last = Some(now);
        match step {
            RecStep::Text(c) => self.text.push(c),
            RecStep::Tap(t) => {
                self.flush_text();
                self.steps.push(format!("tap:{t}"));
            }
        }
    }

    pub(crate) fn finish(mut self) -> String {
        self.flush_text();
        self.steps.join(",")
    }

    pub(crate) fn so_far(&self) -> String {
        let mut c = Recorder { steps: self.steps.clone(), text: self.text.clone(), last: None, ignored: 0 };
        c.flush_text();
        describe_steps(&c.steps.join(","))
    }
}

pub(crate) fn recording_rect(area: Rect) -> Rect {
    let w = 64u16.min(area.width);
    let h = 9u16.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

pub(crate) fn draw_recording(f: &mut Frame, r: &Recorder, area: Rect) {
    let rect = recording_rect(area);
    f.render_widget(Clear, rect);
    let dim = Style::new().fg(Color::DarkGray);
    let so_far = r.so_far();
    let lines = vec![
        Line::from(text::recording::PROMPT),
        Line::from(Span::styled(text::recording::PAUSES, dim)),
        Line::from(""),
        Line::from(vec![
            Span::styled(text::recording::SO_FAR, Style::new().fg(Color::Cyan)),
            Span::raw(if r.steps.is_empty() && r.text.is_empty() { text::recording::NOTHING.to_string() } else { so_far }),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(text::recording::DONE, Style::new().fg(Color::Black).bg(Color::Green)),
            Span::raw("  "),
            Span::styled(text::recording::CANCEL, Style::new().fg(Color::Black).bg(Color::Red)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(localize(lines)).block(Block::bordered().title(tr(text::recording::TITLE).into_owned()).border_style(Style::new().fg(Color::Red))),
        rect,
    );
}

impl App {
    /// The "done" and "cancel" buttons of the recording box can be clicked as well as typed (F2, F3).
    pub(crate) fn recording_mouse(&mut self, m: MouseEvent) {
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let r = recording_rect(self.area);
        if m.column <= r.x || m.column + 1 >= r.x + r.width || m.row != r.y + 6 {
            return;
        }
        let off = m.column - r.x - 1;
        let done = tr(text::recording::DONE).chars().count() as u16;
        if off < done {
            self.finish_recording();
        } else if (done + 2..done + 2 + tr(text::recording::CANCEL).chars().count() as u16).contains(&off) {
            self.modal.recording = None;
            self.log.push("recording cancelled".into());
        }
    }
}
