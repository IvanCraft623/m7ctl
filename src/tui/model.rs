//! Data the pages are made of: rows, tabs and messages.

use crate::tui::*;

pub(crate) const FUNCS: &[&str] = &[
    "left",
    "right",
    "middle",
    "back",
    "forward",
    "dpi-loop",
    "dpi-plus",
    "dpi-minus",
    "play-pause",
    "mute",
    "volume-up",
    "volume-down",
    "prev-track",
    "next-track",
    "stop",
    "media-select",
    "mail",
    "calculator",
    "my-computer",
    "copy",
    "paste",
    "cut",
    "select-all",
    "new",
    "save",
    "print",
    "open",
    "undo",
    "close-window",
    "display-desktop",
    "run",
    "minimize",
    "maximize",
    "disable",
    "three-click",
    "tilt-left",
    "tilt-right",
    "switch-effect",
    "switch-polling",
    "lights-toggle",
];

pub(crate) const MODES: &[&str] = &["steady", "cycle", "breathing", "flash", "off"];

pub(crate) const RATES: &[&str] = &["125", "250", "500", "1000"];

pub(crate) const BUTTONS: &[&str] = &["right", "middle", "back", "forward", "dpi"];

pub(crate) const DEFAULT_COLORS: &str = "00ff00,0000ff,ffa000,ff00ff,00ffff,ff0000,ffff00,ffffff";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Choice(usize),
    Num(i64),
    Text(String),
    Bool(bool),
    Action,
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) label: String,
    pub(crate) value: Value,
    /// Choice labels, or (min, max, step) for numbers.
    pub(crate) opts: Vec<String>,
    pub(crate) range: (i64, i64, i64),
    pub(crate) group: u8,
    /// Free text kept next to a choice (the custom button function).
    pub(crate) extra: String,
}

pub(crate) fn choice(label: &str, opts: &[&str], idx: usize, group: u8) -> Row {
    Row { label: label.into(), value: Value::Choice(idx), opts: opts.iter().map(|s| s.to_string()).collect(), range: (0, 0, 0), group, extra: String::new() }
}

pub(crate) fn num(label: &str, v: i64, min: i64, max: i64, step: i64, group: u8) -> Row {
    Row { label: label.into(), value: Value::Num(v), opts: vec![], range: (min, max, step), group, extra: String::new() }
}

pub(crate) fn text(label: &str, v: &str, group: u8) -> Row {
    Row { label: label.into(), value: Value::Text(v.into()), opts: vec![], range: (0, 0, 0), group, extra: String::new() }
}

pub(crate) fn toggle(label: &str, v: bool, group: u8) -> Row {
    Row { label: label.into(), value: Value::Bool(v), opts: vec![], range: (0, 0, 0), group, extra: String::new() }
}

pub(crate) fn action(label: &str, group: u8) -> Row {
    Row { label: label.into(), value: Value::Action, opts: vec![], range: (0, 0, 0), group, extra: String::new() }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum TabId {
    Status,
    Dpi,
    Rate,
    Lights,
    Buttons,
    Macros,
    Power,
    Profiles,
}

pub(crate) const TABS: &[(TabId, &str)] = &[
    (TabId::Status, text::tabs::STATUS),
    (TabId::Dpi, text::tabs::DPI),
    (TabId::Rate, text::tabs::RATE),
    (TabId::Lights, text::tabs::LIGHTS),
    (TabId::Buttons, text::tabs::BUTTONS),
    (TabId::Macros, text::tabs::MACROS),
    (TabId::Power, text::tabs::POWER),
    (TabId::Profiles, text::tabs::PROFILES),
];

pub(crate) struct Tab {
    pub(crate) id: TabId,
    pub(crate) rows: Vec<Row>,
    pub(crate) sel: usize,
    /// Values as last applied to the mouse (or as recorded at start); rows that differ are marked with `*`.
    pub(crate) applied: Vec<(Value, String)>,
}

impl Tab {
    pub(crate) fn dirty_row(&self, i: usize) -> bool {
        !matches!(self.rows[i].value, Value::Action) && self.applied.get(i).map(|a| a.0 != self.rows[i].value || a.1 != self.rows[i].extra).unwrap_or(false)
    }
    pub(crate) fn dirty(&self) -> bool {
        (0..self.rows.len()).any(|i| self.dirty_row(i))
    }
}

#[derive(Default, Clone)]
pub(crate) struct StatusInfo {
    pub(crate) device: String,
    pub(crate) link: String,
    pub(crate) model: String,
    pub(crate) battery: String,
    pub(crate) charge: String,
    pub(crate) online: String,
    pub(crate) error: Option<String>,
}

pub(crate) enum Msg {
    Applied(usize, u8),
    Line(String),
    Status(StatusInfo),
    /// The mouse reported a DPI-stage change (stage 1-based).
    Dpi(u8, u32),
    /// Connection changed: "wired", "wireless" or "" when no mouse is found.
    Link(String),
    Done,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum NameKind {
    SaveAs,
    Rename(String),
    /// Path of a vendor `.psf` file to import.
    ImportPath,
}

pub(crate) fn is_custom_choice(row: &Row) -> bool {
    matches!(row.value, Value::Choice(i) if row.opts.get(i).map(String::as_str) == Some("custom..."))
}

pub(crate) fn is_color_row(row: &Row) -> bool {
    matches!(row.value, Value::Text(_)) && row.label.contains("olour")
}

pub(crate) fn state_opt(state: &[(String, Vec<String>)], key: &str, opt: &str) -> Option<i64> {
    let v = &state.iter().find(|(k, _)| k == key)?.1;
    v.iter().position(|a| a == opt).and_then(|i| v.get(i + 1)).and_then(|x| x.parse().ok())
}

pub(crate) fn colors_valid(s: &str) -> bool {
    !s.trim().is_empty()
        && s.split(',').all(|c| {
            let h = c.trim().trim_start_matches('#');
            h.len() == 6 && h.chars().all(|ch| ch.is_ascii_hexdigit())
        })
}

pub(crate) fn state_arg(state: &[(String, Vec<String>)], key: &str, i: usize) -> Option<String> {
    state.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.get(i).cloned())
}

/// What the mouse reports about itself.
pub(crate) struct Live {
    pub(crate) status: StatusInfo,
    /// DPI stage the mouse reported last (1-based); unknown until the DPI button is pressed.
    pub(crate) live_stage: Option<u8>,
    /// Last seen connection (None until the first check).
    pub(crate) conn: Option<String>,
    /// Desktop popup on DPI changes (shared with the watcher thread).
    pub(crate) popup: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) last_status: std::time::Instant,
}

/// The Profiles page.
pub(crate) struct ProfileState {
    pub(crate) names: Vec<String>,
    /// Settings stored in each profile (same order as `names`).
    pub(crate) info: Vec<State>,
    pub(crate) sel: usize,
    pub(crate) active: Option<String>,
    /// The active profile file differs from the current settings record.
    pub(crate) modified: bool,
    pub(crate) autosave: bool,
}

/// Open dialogs and text prompts.
pub(crate) struct Modals {
    pub(crate) picker: Option<Picker>,
    pub(crate) chooser: Option<Chooser>,
    pub(crate) step_prompt: Option<StepPrompt>,
    pub(crate) recording: Option<Recorder>,
    /// Profile name being typed.
    pub(crate) naming: Option<String>,
    pub(crate) naming_kind: NameKind,
    pub(crate) confirm_delete: bool,
    /// Reasons to ask before quitting.
    pub(crate) quit_prompt: Option<Vec<String>>,
    pub(crate) editing_custom: bool,
    pub(crate) help: bool,
}

impl Default for Modals {
    fn default() -> Modals {
        Modals {
            picker: None,
            chooser: None,
            step_prompt: None,
            recording: None,
            naming: None,
            naming_kind: NameKind::SaveAs,
            confirm_delete: false,
            quit_prompt: None,
            editing_custom: false,
            help: false,
        }
    }
}

/// Background command execution.
pub(crate) struct Background {
    pub(crate) busy: bool,
    pub(crate) tx: std::sync::mpsc::Sender<Msg>,
    pub(crate) rx: std::sync::mpsc::Receiver<Msg>,
    pub(crate) runner: std::sync::Arc<dyn Runner>,
    /// Run background work on the calling thread (tests).
    pub(crate) inline: bool,
    /// A profile was loaded: show the values it wrote once the commands finish.
    pub(crate) reload_after: bool,
}
