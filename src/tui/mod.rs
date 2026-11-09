//! `m7ctl tui`: the terminal interface.
//!
//! Layout of this module:
//! - [`model`]: rows, tabs and the messages sent from background threads
//! - [`pages`]: one file per page (status, DPI, macros, profiles; the other pages are plain rows)
//! - [`widgets`]: colour picker, list chooser, key recorder, quit/help dialogs
//! - `actions`: rows -> command lines, run in the background through a [`Runner`]
//! - `input`: keyboard and mouse; `view`: drawing
//!
//! Applying a row runs the same command line the CLI takes, in this process, so validation, retries
//! while the mouse sleeps and the settings record behave exactly like the command line.

use crate::cli::Context;
use crate::settings::profiles::summarize;
use crate::settings::State;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

mod actions;
mod i18n;
mod input;
mod model;
mod pages;
#[cfg(test)]
mod tests;
mod text;
mod view;
mod widgets;

use actions::*;
use i18n::{localize, tr, tr_ctx};
use model::*;
use pages::*;
use view::*;
use widgets::*;

struct App {
    tabs: Vec<Tab>,
    cur: usize,
    editing: bool,
    /// Digits being typed into the selected numeric row (Enter confirms, Esc cancels).
    typing: Option<String>,
    log: Vec<String>,
    /// What m7ctl last wrote to the mouse (for the main page).
    cur_state: State,
    /// DPI values of all eight stages, so shrinking and growing the stage count keeps them.
    dpi_mem: [i64; 8],
    quit: bool,
    area: Rect,
    last_click: Option<(std::time::Instant, usize, usize)>,
    ctx: Context,
    /// What the mouse reports about itself (status, DPI stage, connection) and the popup switch.
    live: Live,
    prof: ProfileState,
    /// At most one dialog is open at a time; these are their states.
    modal: Modals,
    /// Commands running in the background and their results.
    bg: Background,
}

impl App {
    fn new(ctx: Context, runner: Arc<dyn Runner>, inline: bool) -> App {
        let (tx, rx) = channel();
        let st = ctx.store.read_state();
        // DPI stages from the record, when there is one.
        let stages: Vec<i64> = state_arg(&st, "dpi", 0)
            .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
            .filter(|v: &Vec<i64>| !v.is_empty())
            .unwrap_or_else(|| vec![800, 1600, 2400, 3200]);
        let active = st
            .iter()
            .find(|(k, _)| k == "dpi")
            .and_then(|(_, v)| v.iter().position(|a| a == "--active").and_then(|i| v.get(i + 1)).and_then(|x| x.parse::<i64>().ok()))
            .unwrap_or(1);
        let mut dpi_mem = [800i64; 8];
        for i in 0..8 {
            dpi_mem[i] = stages.get(i).copied().unwrap_or_else(|| if i == 0 { 800 } else { (dpi_mem[i - 1] + 800).clamp(50, 26000) });
        }
        let colors = pad_colors(&state_arg(&st, "dpi-colors", 0).unwrap_or_else(|| DEFAULT_COLORS.into()));
        let dpi = dpi_rows(stages.len().clamp(1, 8), active, &dpi_mem, &colors);

        let rate = state_arg(&st, "rate", 0).and_then(|r| RATES.iter().position(|x| *x == r)).unwrap_or(2);
        let rates = vec![choice("Polling rate (Hz)", RATES, rate, 0), action("Apply", 0)];

        let mode = state_arg(&st, "light", 0).and_then(|m| MODES.iter().position(|x| *x == m)).unwrap_or(0);
        let lcol = state_arg(&st, "light", 1).filter(|c| c.contains(|ch: char| ch.is_ascii_hexdigit())).unwrap_or_else(|| "0000ff".into());
        let lights = vec![
            choice("Mode", MODES, mode, 0),
            text("Colours", &lcol, 0),
            num("Brightness", state_opt(&st, "light", "--brightness").unwrap_or(3).clamp(0, 3), 0, 3, 1, 0),
            num("Cycle speed", state_opt(&st, "light", "--speed").unwrap_or(1).clamp(0, 3), 0, 3, 1, 0),
            action("Apply", 0),
        ];

        let mut button_opts: Vec<&str> = FUNCS.to_vec();
        button_opts.push("custom...");
        let buttons: Vec<Row> = BUTTONS
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let default = if *b == "dpi" { 5 } else { FUNCS.iter().position(|f| f == b).unwrap_or(0) };
                let (idx, extra) = match state_arg(&st, &format!("button:{b}"), 1) {
                    Some(f) => match FUNCS.iter().position(|x| *x == f) {
                        Some(p) => (p, String::new()),
                        None => (FUNCS.len(), f),
                    },
                    None => (default, String::new()),
                };
                let mut r = choice(&format!("{b} button"), &button_opts, idx, i as u8);
                r.extra = extra;
                r
            })
            .collect();

        let macro_slot = 1i64;
        let macros = macro_rows(macro_slot, &macro_steps(&st, macro_slot));

        let power = vec![
            num("Power-down (minutes)", state_arg(&st, "power", 0).and_then(|v| v.parse().ok()).unwrap_or(6), 3, 10, 1, 0),
            toggle("Lights off while moving", st.iter().any(|(k, v)| k == "power" && v.iter().any(|a| a == "lights-off-moving")), 0),
            action("Apply", 0),
            toggle("Motion wake-up", state_arg(&st, "motion-wake", 0).map(|v| v != "off").unwrap_or(true), 1),
            action("Apply", 1),
            choice("Lift-off distance (mm)", &["1", "2"], 1, 2),
            action("Apply", 2),
        ];

        let mk = |id, rows: Vec<Row>| Tab { id, sel: 0, applied: rows.iter().map(|r| (r.value.clone(), r.extra.clone())).collect(), rows };
        let tabs = vec![
            mk(TabId::Status, vec![]),
            mk(TabId::Dpi, dpi),
            mk(TabId::Rate, rates),
            mk(TabId::Lights, lights),
            mk(TabId::Buttons, buttons),
            mk(TabId::Macros, macros),
            mk(TabId::Power, power),
            mk(TabId::Profiles, vec![]),
        ];
        let mut app = App {
            tabs,
            cur: 0,
            editing: false,
            typing: None,
            log: vec![],
            cur_state: vec![],
            dpi_mem,
            quit: false,
            area: Rect::new(0, 0, 110, 34),
            last_click: None,
            live: Live {
                status: StatusInfo::default(),
                live_stage: None,
                conn: None,
                popup: Arc::new(std::sync::atomic::AtomicBool::new(ctx.store.popup())),
                last_status: std::time::Instant::now(),
            },
            prof: ProfileState { names: vec![], info: vec![], sel: 0, active: None, modified: false, autosave: true },
            modal: Modals::default(),
            bg: Background { busy: false, tx, rx, runner, inline, reload_after: false },
            ctx,
        };
        app.refresh_profiles();
        if let Some(a) = &app.prof.active {
            app.prof.sel = app.prof.names.iter().position(|p| p == a).unwrap_or(0);
        }
        app
    }

    fn tab(&self) -> &Tab {
        &self.tabs[self.cur]
    }
}

pub fn run(ctx: Context) -> Result<(), String> {
    i18n::init(ctx.store.language().as_deref());
    let mut app = App::new(ctx.clone(), Arc::new(InProcess(ctx)), false);
    // progress messages ("the mouse is asleep...") belong in the log, not on the terminal
    let (tx, notice_ctx) = (std::sync::Mutex::new(app.bg.tx.clone()), &mut app.ctx);
    notice_ctx.notice = Arc::new(move |m| {
        let _ = tx.lock().map(|t| t.send(Msg::Line(m.to_string())));
    });
    let runner_ctx = app.ctx.clone();
    app.bg.runner = Arc::new(InProcess(runner_ctx));
    let mut terminal: DefaultTerminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
        prev(info);
    }));
    app.spawn_dpi_watcher();
    let res = main_loop(&mut terminal, &mut app);
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    res.map_err(|e| e.to_string())
}

const STATUS_POLL_VISIBLE: Duration = Duration::from_secs(3);
const STATUS_POLL_HIDDEN: Duration = Duration::from_secs(60);

fn main_loop(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    app.refresh_status();
    while !app.quit {
        app.drain();
        // Plugging the charging cable changes the reply within seconds, so the Status page follows it closely.
        let every = if app.tab().id == TabId::Status { STATUS_POLL_VISIBLE } else { STATUS_POLL_HIDDEN };
        if !app.bg.busy && app.live.last_status.elapsed() > every {
            app.live.last_status = std::time::Instant::now();
            app.poll_status();
        }
        app.area = Rect::new(0, 0, terminal.size()?.width, terminal.size()?.height);
        terminal.draw(|f| draw(f, app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) => app.key(k),
                Event::Mouse(m) => app.mouse(m),
                _ => {}
            }
        }
    }
    Ok(())
}
