//! TUI tests: key and mouse flows, and what the screens show.

use super::*;
use ratatui::{backend::TestBackend, Terminal};
use std::sync::Mutex;

/// Remembers every command line it is given instead of running it.
#[derive(Default)]
struct CommandLog(Mutex<Vec<Vec<String>>>);

impl Runner for CommandLog {
    fn run(&self, args: &[String]) -> Result<String, String> {
        self.0.lock().unwrap().push(args.to_vec());
        Ok(String::new())
    }
    fn log(&self) -> Vec<Vec<String>> {
        self.0.lock().unwrap().clone()
    }
}

impl App {
    /// An app with an empty temporary configuration, a fake mouse and a command recorder.
    fn for_test() -> App {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("m7ctl-tui-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Context {
            store: crate::settings::Store::at(dir),
            device: None,
            opener: Arc::new(|_| {
                let (t, _) = crate::device::fake::FakeTransport::new();
                Ok(crate::device::Mouse::new(Box::new(t)).without_delays())
            }),
            notice: Arc::new(|_| {}),
        };
        App::new(ctx, Arc::new(CommandLog::default()), true)
    }

    fn sent(&self) -> Vec<Vec<String>> {
        self.bg.runner.log()
    }
}

fn screen(app: &App) -> String {
    let mut t = Terminal::new(TestBackend::new(110, 34)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    buf.content().chunks(110).map(|r| r.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

#[test]
fn dpi_commands_follow_the_rows() {
    let app = App::for_test();
    let c = app.commands(TabId::Dpi, 0, "").unwrap();
    assert_eq!(c[0][0], "dpi");
    assert_eq!(c[0][2], "--active");
    let n = c[0][1].split(',').count();
    assert!((1..=6).contains(&n));
}

#[test]
fn keys_change_values_and_build_commands() {
    let mut app = App::for_test();
    app.cur = 2; // polling
    app.key(key(KeyCode::Right)); // 500 -> 1000
    let c = app.commands(TabId::Rate, 0, "").unwrap();
    assert_eq!(c, vec![vec!["rate".to_string(), "1000".to_string()]]);
    app.cur = 6; // power
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Down));
    app.key(key(KeyCode::Right)); // lights off while moving on
    let c = app.commands(TabId::Power, 0, "").unwrap();
    assert_eq!(c[0][0], "power");
    assert_eq!(c[0].last().unwrap(), "lights-off-moving");
}

#[test]
fn power_stays_in_range() {
    let mut app = App::for_test();
    app.cur = 6;
    for _ in 0..30 {
        app.key(key(KeyCode::Right));
    }
    match &app.tabs[6].rows[0].value {
        Value::Num(n) => assert_eq!(*n, 10),
        _ => panic!(),
    }
}

#[test]
fn lights_cycle_command() {
    let mut app = App::for_test();
    app.cur = 3;
    app.key(key(KeyCode::Right)); // steady -> cycle
    let c = app.commands(TabId::Lights, 0, "").unwrap();
    assert_eq!(c[0][..2], ["light".to_string(), "cycle".to_string()]);
    assert!(c[0].contains(&"--speed".to_string()));
}

#[test]
fn button_command_uses_the_selected_function() {
    let mut app = App::for_test();
    app.cur = 4;
    app.tabs[4].sel = 3; // forward
    app.key(key(KeyCode::Right));
    let label = app.tabs[4].rows[3].label.clone();
    let c = app.commands(TabId::Buttons, 3, &label).unwrap();
    assert_eq!(c[0][0..2], ["button".to_string(), "forward".to_string()]);
}

#[test]
fn custom_button_function_flow() {
    let mut app = App::for_test();
    app.cur = 4;
    app.tabs[4].sel = 2; // back button
    let n = FUNCS.len();
    // stepping onto "custom..." starts typing
    for _ in 0..(n + 1
        - match app.tabs[4].rows[2].value {
            Value::Choice(i) => i,
            _ => 0,
        })
    {
        app.key(key(KeyCode::Right));
        if app.modal.editing_custom {
            break;
        }
    }
    assert!(app.modal.editing_custom);
    assert!(app.commands(TabId::Buttons, 2, "back button").is_err(), "an empty custom function must be refused");
    for c in "key:ctrl+c".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    app.key(key(KeyCode::Enter));
    assert!(!app.modal.editing_custom);
    assert_eq!(app.tabs[4].rows[2].extra, "key:ctrl+c");
    assert!(app.tabs[4].dirty_row(2));
    let c = app.commands(TabId::Buttons, 2, "back button").unwrap();
    assert_eq!(c, vec![vec!["button".to_string(), "back".to_string(), "key:ctrl+c".to_string()]]);
}

#[test]
fn profile_name_entry() {
    let mut app = App::for_test();
    app.cur = 7;
    app.key(key(KeyCode::Char('s')));
    assert!(app.modal.naming.is_some());
    app.key(key(KeyCode::Char('a')));
    app.key(key(KeyCode::Char('/')));
    app.key(key(KeyCode::Char('1')));
    assert_eq!(app.modal.naming.as_deref(), Some("a1"));
}

fn click(app: &mut App, col: u16, row: u16) {
    app.mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: KeyModifiers::NONE });
}

#[test]
fn clicking_page_titles_switches_pages() {
    let mut app = App::for_test();
    let r = tab_ranges();
    for i in [3usize, 5, 1, 6, 0] {
        click(&mut app, 1 + r[i].0 + 2, 1);
        assert_eq!(app.cur, i);
    }
}

#[test]
fn clicking_rows_and_arrows() {
    let mut app = App::for_test();
    app.cur = 2; // polling: row 0 = rate (500), row 1 = apply
                 // body inner starts at x=1, y=4
    click(&mut app, 1 + VALUE_X + 1, 4); // left half of "< 500 >": decrease -> 250
    match &app.tabs[2].rows[0].value {
        Value::Choice(i) => assert_eq!(RATES[*i], "250"),
        _ => panic!(),
    }
    click(&mut app, 1 + VALUE_X + 6, 4); // right half: back to 500
    match &app.tabs[2].rows[0].value {
        Value::Choice(i) => assert_eq!(RATES[*i], "500"),
        _ => panic!(),
    }
    click(&mut app, 3, 5); // select the apply row (line 1) and apply it
    assert_eq!(app.tabs[2].sel, 1);
    assert_eq!(app.sent(), vec![vec!["rate".to_string(), "500".to_string()]]);
}

#[test]
fn clicking_a_colour_row_opens_the_picker_not_the_editor() {
    let mut app = App::for_test();
    app.cur = 3;
    click(&mut app, 5, 5); // line 1 = colours
    assert!(app.modal.picker.is_some() && !app.editing);
}

#[test]
fn wheel_changes_values_and_moves_selection() {
    let mut app = App::for_test();
    app.cur = 6; // power: row 0 = minutes (default 6)
    let ev = |kind, col| MouseEvent { kind, column: col, row: 4, modifiers: KeyModifiers::NONE };
    app.mouse(ev(MouseEventKind::ScrollUp, 1 + VALUE_X + 1));
    match &app.tabs[6].rows[0].value {
        Value::Num(n) => assert_eq!(*n, 7),
        _ => panic!(),
    }
    app.mouse(ev(MouseEventKind::ScrollDown, 5)); // over the label: selection moves down
    assert_eq!(app.tabs[6].sel, 1);
}

#[test]
fn colour_lists_are_validated() {
    assert!(colors_valid("ff0000,00ff00"));
    assert!(colors_valid("#ff8000"));
    assert!(!colors_valid("ff00"));
    assert!(!colors_valid(""));
    assert!(!colors_valid("ff0000,zzzzzz"));
}

fn picker_click(app: &mut App, off: u16, line: u16) {
    let r = picker_rect(app.area);
    click(app, r.x + 1 + off, r.y + 1 + line);
}

fn open_lights_picker() -> App {
    let mut app = App::for_test();
    app.cur = 3;
    app.tabs[3].sel = 1;
    app.tabs[3].rows[1].value = Value::Text("0000ff".into());
    app.key(key(KeyCode::Enter));
    assert!(app.modal.picker.is_some());
    app
}

#[test]
fn hsl_math() {
    assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), [255, 0, 0]);
    assert_eq!(hsl_to_rgb(120.0, 1.0, 0.5), [0, 255, 0]);
    assert_eq!(hsl_to_rgb(240.0, 1.0, 0.5), [0, 0, 255]);
    assert_eq!(hsl_to_rgb(0.0, 0.0, 1.0), [255, 255, 255]);
    assert_eq!(parse_color_list("ff0000, 00ff00"), vec![[255, 0, 0], [0, 255, 0]]);
    assert_eq!(parse_color_list("junk"), vec![[0, 0, 255]]);
}

#[test]
fn palette_click_then_ok_writes_the_row() {
    let mut app = open_lights_picker();
    picker_click(&mut app, 0, L_PAL + 4); // hue 0, lightness 0.56 -> a red
    let c = app.modal.picker.as_ref().unwrap().colors[0];
    assert!(c[0] > 200 && c[1] < 100 && c[2] < 100, "{c:?}");
    picker_click(&mut app, OK_X + 1, L_BUTTONS);
    assert!(app.modal.picker.is_none());
    match &app.tabs[3].rows[1].value {
        Value::Text(t) => assert_eq!(t, &hex_of(c)),
        _ => panic!(),
    }
}

#[test]
fn cancel_and_escape_keep_the_old_colour() {
    let mut app = open_lights_picker();
    picker_click(&mut app, 3, L_PRESETS); // a preset
    picker_click(&mut app, CANCEL_X + 1, L_BUTTONS);
    assert!(app.modal.picker.is_none());
    assert_eq!(app.tabs[3].rows[1].value, Value::Text("0000ff".into()));
    let mut app = open_lights_picker();
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Esc));
    assert_eq!(app.tabs[3].rows[1].value, Value::Text("0000ff".into()));
}

#[test]
fn sliders_keys_and_colour_list() {
    let mut app = open_lights_picker();
    picker_click(&mut app, BAR_X + BAR_W - 1, L_BARS); // R to max
    assert_eq!(app.modal.picker.as_ref().unwrap().colors[0], [255, 0, 255]);
    app.key(key(KeyCode::Down)); // channel G
    for _ in 0..3 {
        app.key(key(KeyCode::PageUp));
    }
    assert_eq!(app.modal.picker.as_ref().unwrap().colors[0], [255, 48, 255]);
    app.key(key(KeyCode::Char('+')));
    app.key(key(KeyCode::Char('+')));
    assert_eq!(app.modal.picker.as_ref().unwrap().colors.len(), 3);
    app.key(key(KeyCode::Char('-')));
    assert_eq!(app.modal.picker.as_ref().unwrap().colors.len(), 2);
    app.key(key(KeyCode::Enter));
    assert_eq!(app.tabs[3].rows[1].value, Value::Text("ff30ff,ff30ff".into()));
}

#[test]
fn clicking_the_chips_adds_selects_and_removes() {
    let mut app = open_lights_picker();
    let end = 7 + 4; // one chip
    picker_click(&mut app, end + 2, L_CHIPS); // [ + ]
    assert_eq!(app.modal.picker.as_ref().unwrap().colors.len(), 2);
    picker_click(&mut app, 8, L_CHIPS); // chip 1
    assert_eq!(app.modal.picker.as_ref().unwrap().idx, 0);
    let end = 7 + 8;
    picker_click(&mut app, end + 8, L_CHIPS); // [ - ]
    assert_eq!(app.modal.picker.as_ref().unwrap().colors.len(), 1);
}

#[test]
fn e_key_still_types_hex_on_a_colour_row() {
    let mut app = App::for_test();
    app.cur = 3;
    app.tabs[3].sel = 1;
    app.key(key(KeyCode::Char('e')));
    assert!(app.editing && app.modal.picker.is_none());
}

#[test]
fn changed_rows_are_marked_until_applied() {
    let mut app = App::for_test();
    app.cur = 2; // polling
    assert!(!app.tabs[2].dirty());
    app.key(key(KeyCode::Right));
    assert!(app.tabs[2].dirty() && app.tabs[2].dirty_row(0));
    assert!(screen(&app).contains("Rate*"), "{}", screen(&app));
    // the worker reports success: the page is clean again
    app.bg.tx.send(Msg::Applied(2, 0)).unwrap();
    app.drain();
    assert!(!app.tabs[2].dirty());
}

#[test]
fn clicking_a_stage_colour_opens_the_picker_on_that_stage() {
    let mut app = App::for_test();
    app.cur = 1;
    // row 3 = "Stage 2 DPI" (rows: stages in use, active stage, stage 1, stage 2, ...)
    click(&mut app, 1 + VALUE_X + 15, 4 + 3);
    let p = app.modal.picker.as_ref().expect("picker should open");
    assert_eq!(p.idx, 1);
    // OK writes back into the scroll-light colour row, not the DPI value
    app.key(key(KeyCode::Enter));
    assert!(app.modal.picker.is_none());
}

#[test]
fn home_and_end_jump() {
    let mut app = App::for_test();
    app.cur = 1;
    app.key(key(KeyCode::End));
    assert_eq!(app.tabs[1].sel, app.tabs[1].rows.len() - 1);
    app.key(key(KeyCode::Home));
    assert_eq!(app.tabs[1].sel, 0);
}

fn profiles_app(active: Option<&str>, modified: bool, autosave: bool) -> App {
    let mut app = App::for_test();
    app.cur = 7;
    app.prof.names = vec!["gaming".into(), "work".into()];
    app.prof.info = vec![
        vec![("rate".into(), vec!["1000".into()]), ("light".into(), vec!["steady".into(), "ff0000".into()])],
        vec![("rate".into(), vec!["500".into()]), ("dpi".into(), vec!["800,1600".into(), "--active".into(), "1".into()])],
    ];
    app.prof.active = active.map(String::from);
    app.prof.modified = modified;
    app.prof.autosave = autosave;
    app
}

/// Column (relative to the page) where profile button `id` starts.
fn button_x(app: &App, id: u8) -> u16 {
    let mut x = 0;
    for (label, _, i) in app.profile_buttons() {
        if i == id {
            return x;
        }
        x += label.chars().count() as u16 + 2;
    }
    panic!()
}

#[test]
fn save_as_new_asks_for_a_name_and_refuses_duplicates() {
    let mut app = profiles_app(Some("work"), false, true);
    let x = 1 + button_x(&app, 2) + 2;
    click(&mut app, x, 4 + 3); // [ Save as new... ]
    assert!(app.modal.naming.is_some());
    for c in "gaming".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    app.key(key(KeyCode::Enter));
    assert!(app.sent().is_empty(), "an existing name must not be overwritten silently");
    assert!(app.log.last().unwrap().contains("already exists"));
    app.start_naming(NameKind::SaveAs);
    for c in "night".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    app.key(key(KeyCode::Enter));
    assert_eq!(app.sent(), vec![vec!["profile".to_string(), "save".into(), "night".into()]]);
}

#[test]
fn save_button_overwrites_the_active_profile() {
    let mut app = profiles_app(Some("work"), true, false);
    let x = 1 + button_x(&app, 1) + 2;
    click(&mut app, x, 4 + 3);
    assert_eq!(app.sent(), vec![vec!["profile".to_string(), "save".into(), "work".into()]]);
    // without an active profile it asks for a name instead
    let mut app = profiles_app(None, false, true);
    app.key(key(KeyCode::Char('s')));
    assert!(app.modal.naming.is_some() && app.sent().is_empty());
}

#[test]
fn rename_load_delete_and_autosave_toggle() {
    let mut app = profiles_app(Some("work"), false, true);
    app.prof.sel = 1;
    app.key(key(KeyCode::Char('r')));
    assert_eq!(app.modal.naming.as_deref(), Some("work"));
    for _ in 0..4 {
        app.key(key(KeyCode::Backspace));
    }
    for c in "office".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    app.key(key(KeyCode::Enter));
    assert_eq!(app.sent().last().unwrap(), &vec!["profile".to_string(), "rename".into(), "work".into(), "office".into()]);
    app.bg.busy = false;
    app.key(key(KeyCode::Char('l')));
    assert_eq!(app.sent().last().unwrap(), &vec!["profile".to_string(), "load".into(), "work".into()]);
    app.bg.busy = false;
    app.key(key(KeyCode::Char('d')));
    assert!(app.modal.confirm_delete);
    app.key(key(KeyCode::Char('d')));
    assert_eq!(app.sent().last().unwrap(), &vec!["profile".to_string(), "delete".into(), "work".into()]);
    assert!(app.prof.autosave);
    app.key(key(KeyCode::Char('A')));
    assert!(!app.prof.autosave);
    click(&mut app, 3, 4 + 1); // the auto-save line
    assert!(app.prof.autosave);
}

#[test]
fn quitting_asks_only_when_something_would_be_lost() {
    let mut app = App::for_test();
    app.key(key(KeyCode::Char('q')));
    assert!(app.quit, "a clean session quits at once");
    let mut app = App::for_test();
    app.cur = 2;
    app.key(key(KeyCode::Right)); // unapplied change on Polling
    app.key(key(KeyCode::Char('q')));
    assert!(!app.quit && app.modal.quit_prompt.is_some());
    let s = screen(&app);
    assert!(s.contains("Not applied to the mouse yet: Rate") && s.contains("[ Quit (y) ]") && s.contains("[ Stay ]"), "{s}");
    assert!(!s.contains("Save and quit"));
    app.key(key(KeyCode::Char('x'))); // any other key: stay
    assert!(app.modal.quit_prompt.is_none() && !app.quit);
    app.key(key(KeyCode::Char('q')));
    app.key(key(KeyCode::Char('y')));
    assert!(app.quit);
}

#[test]
fn unsaved_profile_changes_offer_to_save_on_quit() {
    let mut app = profiles_app(Some("work"), true, false);
    app.key(key(KeyCode::Char('q')));
    let s = screen(&app);
    assert!(s.contains("Profile 'work' has unsaved changes") && s.contains("[ Save and quit (s) ]"), "{s}");
    // with auto-save on the same situation does not block quitting
    let mut app = profiles_app(Some("work"), true, true);
    app.key(key(KeyCode::Char('q')));
    assert!(app.quit);
}

#[test]
fn quit_prompt_buttons_are_clickable() {
    let mut app = App::for_test();
    app.cur = 2;
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Char('q')));
    let r = quit_rect(app.area, 1);
    // the button line is two lines below the single reason
    click(&mut app, r.x + 1 + 2, r.y + 1 + 2);
    assert!(app.quit, "[ Quit anyway ] is the first button");
    let mut app = App::for_test();
    app.cur = 2;
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Char('q')));
    let r = quit_rect(app.area, 1);
    click(&mut app, r.x + 1 + 15 + 2 + 2, r.y + 1 + 2); // [ Stay ]
    assert!(!app.quit && app.modal.quit_prompt.is_none());
}

#[test]
fn a_applies_and_u_undoes() {
    let mut app = App::for_test();
    app.cur = 2;
    app.key(key(KeyCode::Right));
    assert!(app.tabs[2].dirty());
    app.key(key(KeyCode::Char('u')));
    assert!(!app.tabs[2].dirty());
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Char('a')));
    assert_eq!(app.sent(), vec![vec!["rate".to_string(), "1000".to_string()]]);
}

#[test]
fn dpi_apply_keeps_live_stage_unless_starting_stage_edited() {
    let mut app = profiles_app(None, false, true);
    app.live.live_stage = Some(3);
    let n = app.tabs.iter().position(|t| t.id == TabId::Dpi).unwrap();
    let t = &mut app.tabs[n];
    let i = t.rows.iter().position(|r| r.label.starts_with("Stages in use")).unwrap();
    t.rows[i].value = Value::Num(4);
    let cmds = app.commands(TabId::Dpi, 0, "Apply").unwrap();
    assert!(cmds[0].len() == 4 && cmds[0][2] == "--active", "{:?}", cmds[0]);
    assert_eq!(cmds[0][3], "3");
}

#[test]
fn connection_changes_are_logged_and_clear_status() {
    let mut app = profiles_app(None, false, true);
    app.bg.tx.send(Msg::Link("wired".into())).unwrap();
    app.drain();
    assert_eq!(app.live.conn.as_deref(), Some("wired"));
    assert!(!app.log.iter().any(|l| l.contains("connected")), "first check is silent");
    app.live.live_stage = Some(2);
    app.bg.tx.send(Msg::Link("".into())).unwrap();
    app.drain();
    assert_eq!(app.log.last().map(String::as_str), Some("mouse disconnected"));
    assert!(app.live.live_stage.is_none());
    assert!(screen(&app).contains("Mouse not connected") || app.live.conn.as_deref() == Some(""));
    app.bg.tx.send(Msg::Link("wireless".into())).unwrap();
    app.drain();
    assert_eq!(app.log.last().map(String::as_str), Some("mouse connected (wireless)"));
}

#[test]
fn dpi_event_updates_live_stage_and_popup_toggles() {
    let mut app = profiles_app(None, false, true);
    app.bg.tx.send(Msg::Dpi(3, 2400)).unwrap();
    app.drain();
    assert_eq!(app.live.live_stage, Some(3));
    assert_eq!(app.log.last().map(String::as_str), Some("DPI stage 3: 2400"));
    let before = app.live.popup.load(std::sync::atomic::Ordering::Relaxed);
    press(&mut app, KeyCode::Char('p'));
    assert_eq!(app.live.popup.load(std::sync::atomic::Ordering::Relaxed), !before);
}

#[test]
fn autosave_target_is_chosen_when_applying() {
    // the worker saves into the active profile only when auto-save is on and a profile is active
    let mut app = profiles_app(Some("work"), false, true);
    assert!(app.prof.autosave && app.prof.active.is_some());
    app.prof.autosave = false;
    assert!(!(app.prof.autosave && app.prof.active.is_some()));
}

#[test]
fn dpi_page_lists_only_the_stages_in_use_and_remembers_the_rest() {
    let mut app = App::for_test();
    app.cur = 1;
    let stage_rows = |a: &App| a.tabs[1].rows.iter().filter(|r| stage_number(&r.label).is_some()).count();
    assert_eq!(stage_rows(&app), 4);
    let before: Vec<Value> = app.tabs[1].rows.iter().filter(|r| stage_number(&r.label).is_some()).map(|r| r.value.clone()).collect();
    app.key(key(KeyCode::Left)); // 4 -> 3 stages
    app.key(key(KeyCode::Left)); // 3 -> 2
    assert_eq!(stage_rows(&app), 2);
    assert!(!screen(&app).contains("Stage 3 DPI"));
    // the starting stage cannot exceed the stages in use
    assert_eq!(app.tabs[1].rows[1].range.1, 2);
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Right)); // back to 4: the old values are still there
    let after: Vec<Value> = app.tabs[1].rows.iter().filter(|r| stage_number(&r.label).is_some()).map(|r| r.value.clone()).collect();
    assert_eq!(before, after);
    // the commands only mention the stages in use
    app.key(key(KeyCode::Left));
    let c = app.commands(TabId::Dpi, 0, "").unwrap();
    assert_eq!(c[0][1].split(',').count(), 3);
}

#[test]
fn a_new_stage_row_does_not_look_unapplied() {
    let mut app = App::for_test();
    app.cur = 1;
    app.key(key(KeyCode::Right)); // 4 -> 5 stages: only the count changed
    let dirty: Vec<String> = (0..app.tabs[1].rows.len()).filter(|i| app.tabs[1].dirty_row(*i)).map(|i| app.tabs[1].rows[i].label.clone()).collect();
    assert_eq!(dirty, vec!["Stages in use".to_string()]);
}

fn open_chooser_on(row: usize) -> App {
    let mut app = App::for_test();
    app.cur = 4;
    app.tabs[4].sel = row;
    app.key(key(KeyCode::Enter));
    assert!(app.modal.chooser.is_some());
    app
}

#[test]
fn enter_on_a_button_opens_a_filterable_list_and_applies_the_pick() {
    let mut app = open_chooser_on(3); // forward button
    let s = screen(&app);
    assert!(s.contains("Filter:") && s.contains("Enter: choose and apply"), "{s}");
    for c in "volume".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    let items = app.chooser_items();
    assert_eq!(items.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>(), vec!["volume-up", "volume-down"]);
    app.key(key(KeyCode::Down));
    app.key(key(KeyCode::Enter));
    assert!(app.modal.chooser.is_none());
    assert_eq!(app.sent(), vec![vec!["button".to_string(), "forward".into(), "volume-down".into()]]);
    // the row only counts as applied once the worker reports success
    assert!(app.tabs[4].dirty_row(3));
}

#[test]
fn escape_cancels_the_list_and_changes_nothing() {
    let mut app = open_chooser_on(0);
    app.key(key(KeyCode::Char('m')));
    app.key(key(KeyCode::Esc));
    assert!(app.modal.chooser.is_none() && app.sent().is_empty() && !app.tabs[4].dirty());
}

#[test]
fn choosing_custom_starts_typing_instead_of_applying() {
    let mut app = open_chooser_on(2);
    for c in "cust".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    app.key(key(KeyCode::Enter));
    assert!(app.modal.editing_custom && app.sent().is_empty());
}

#[test]
fn clicking_a_list_entry_picks_it() {
    let mut app = open_chooser_on(1);
    for c in "mute".chars() {
        app.key(key(KeyCode::Char(c)));
    }
    let r = chooser_rect(app.area, app.chooser_total());
    click(&mut app, r.x + 3, r.y + 2); // first entry line
    assert!(app.modal.chooser.is_none());
    assert_eq!(app.sent(), vec![vec!["button".to_string(), "middle".into(), "mute".into()]]);
}

#[test]
fn right_click_applies_a_button_without_the_list() {
    let mut app = App::for_test();
    app.cur = 4;
    app.mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Right), column: 5, row: 4 + 2, modifiers: KeyModifiers::NONE });
    assert!(app.modal.chooser.is_none());
    assert_eq!(app.sent().len(), 1);
    assert_eq!(app.sent()[0][..2], ["button".to_string(), "back".to_string()]);
}

#[test]
fn clicking_a_buttons_function_opens_the_list_but_its_label_does_not() {
    let mut app = App::for_test();
    app.cur = 4;
    click(&mut app, 3, 4 + 2); // the label only selects the row
    assert_eq!(app.tabs[4].sel, 2);
    assert!(app.modal.chooser.is_none());
    click(&mut app, 1 + VALUE_X + 1, 4 + 2); // the function opens the list, like Enter
    assert!(app.modal.chooser.is_some());
}

fn macros_app() -> App {
    let mut app = App::for_test();
    app.cur = 5;
    assert_eq!(app.tab().id, TabId::Macros);
    app
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.key(key(KeyCode::Char(c)));
    }
}

#[test]
fn macros_page_stores_a_macro_in_the_chosen_slot() {
    let mut app = macros_app();
    app.key(key(KeyCode::Right)); // slot 2
    app.key(key(KeyCode::Down)); // steps
    app.key(key(KeyCode::Enter)); // start typing
    assert!(app.editing);
    type_text(&mut app, "tap:ctrl+c,wait:100");
    app.key(key(KeyCode::Enter));
    assert!(app.tabs[5].dirty(), "typed steps are not stored yet");
    app.tabs[5].sel = 5; // [ 3. Save to the mouse ]
    app.key(key(KeyCode::Enter));
    assert_eq!(app.sent(), vec![vec!["macro".to_string(), "2".into(), "tap:ctrl+c,wait:100".into()]]);
}

#[test]
fn storing_an_empty_macro_is_refused() {
    let app = macros_app();
    assert!(app.commands(TabId::Macros, 0, "").is_err());
}

#[test]
fn clearing_a_slot_empties_the_steps() {
    let mut app = macros_app();
    app.tabs[5].rows[1].value = Value::Text("type:hi".into());
    app.tabs[5].sel = 10; // [ Clear this slot ]
    app.key(key(KeyCode::Enter));
    assert_eq!(app.sent(), vec![vec!["macro".to_string(), "1".into(), "clear".into()]]);
    assert_eq!(app.tabs[5].rows[1].value, Value::Text(String::new()));
    assert!(!app.tabs[5].dirty_row(1));
}

#[test]
fn assigning_a_macro_builds_the_button_function() {
    let mut app = macros_app();
    let cmd = |a: &App| a.commands(TabId::Macros, 2, "").unwrap();
    // default: forward button, repeat N times, once
    assert_eq!(cmd(&app), vec![vec!["button".to_string(), "forward".into(), "macro:1,2,1".into()]]);
    app.tabs[5].sel = 7; // Repeat
    app.key(key(KeyCode::Left)); // while held
    assert_eq!(cmd(&app)[0][2], "macro:1,1");
    app.key(key(KeyCode::Left)); // loop
    assert_eq!(cmd(&app)[0][2], "macro:1,0");
    assert!(screen(&app).contains("loop never stops by itself"));
    app.key(key(KeyCode::Right));
    app.key(key(KeyCode::Right)); // times
    app.tabs[5].sel = 8; // Times
    for _ in 0..4 {
        app.key(key(KeyCode::Right));
    }
    assert_eq!(cmd(&app)[0][2], "macro:1,2,5");
    assert!(!screen(&app).contains("loop never stops"));
}

#[test]
fn changing_the_slot_shows_what_it_holds_without_marking_a_change() {
    let mut app = macros_app();
    app.cur_state = vec![("macro:2".into(), vec!["2".into(), "type:hello".into()])];
    app.key(key(KeyCode::Right)); // slot 2
    assert_eq!(app.tabs[5].rows[1].value, Value::Text("type:hello".into()));
    assert!(!app.tabs[5].dirty(), "browsing slots is not an unapplied change");
    app.key(key(KeyCode::Right)); // slot 3 is empty
    assert_eq!(app.tabs[5].rows[1].value, Value::Text(String::new()));
}

fn press(app: &mut App, code: KeyCode) {
    app.key(key(code));
}

fn press_mod(app: &mut App, code: KeyCode, m: KeyModifiers) {
    app.key(KeyEvent::new(code, m));
}

#[test]
fn recording_turns_key_presses_into_steps() {
    let mut app = macros_app();
    app.tabs[5].sel = 2; // [ Record keys ]
    press(&mut app, KeyCode::Enter);
    assert!(app.modal.recording.is_some());
    let s = screen(&app);
    assert!(s.contains("Press the keys") && s.contains("F2: done") && s.contains("F3: cancel"), "{s}");
    // 'q' and Esc are recorded, not obeyed, while recording
    press(&mut app, KeyCode::Char('h'));
    press(&mut app, KeyCode::Char('i'));
    press_mod(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('q'));
    assert!(!app.quit);
    assert!(screen(&app).contains("type \"hi\"") && screen(&app).contains("press ctrl+c"));
    press(&mut app, KeyCode::F(2));
    assert!(app.modal.recording.is_none());
    assert_eq!(app.steps_now(), "type:hi,tap:ctrl+c,tap:enter,type:q");
    assert!(app.log.last().unwrap().contains("recorded 4 steps"));
}

#[test]
fn recording_notes_pauses_and_appends_to_existing_steps() {
    let mut app = macros_app();
    app.tabs[5].rows[1].value = Value::Text("tap:a".into());
    let mut r = Recorder::default();
    let t0 = std::time::Instant::now();
    r.key(&key(KeyCode::Char('x')), t0);
    r.key(&key(KeyCode::Char('y')), t0 + Duration::from_millis(100)); // quick: same text
    r.key(&key(KeyCode::Enter), t0 + Duration::from_millis(1100)); // 1 s pause
    assert_eq!(r.finish(), "type:xy,wait:1000,tap:enter");
    app.modal.recording = Some(Recorder { steps: vec!["tap:enter".into()], ..Default::default() });
    press(&mut app, KeyCode::F(2));
    assert_eq!(app.steps_now(), "tap:a,tap:enter");
}

#[test]
fn recording_can_be_cancelled_and_ignores_unstorable_keys() {
    let mut app = macros_app();
    app.modal.recording = Some(Recorder::default());
    press(&mut app, KeyCode::Char('!')); // shifted punctuation cannot be stored
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::F(3));
    assert!(app.modal.recording.is_none());
    assert_eq!(app.steps_now(), "");
    app.modal.recording = Some(Recorder::default());
    press(&mut app, KeyCode::Char('!'));
    press(&mut app, KeyCode::F(2));
    assert_eq!(app.steps_now(), "", "nothing storable was pressed");
    assert!(app.log.last().unwrap().contains("nothing recorded"));
}

#[test]
fn recorded_keys_map_to_valid_macro_steps() {
    let cases = [
        (KeyCode::Backspace, KeyModifiers::NONE, "tap:backspace"),
        (KeyCode::Delete, KeyModifiers::NONE, "tap:delete"),
        (KeyCode::Left, KeyModifiers::CONTROL, "tap:ctrl+left"),
        (KeyCode::F(5), KeyModifiers::NONE, "tap:f5"),
        (KeyCode::Char('v'), KeyModifiers::CONTROL, "tap:ctrl+v"),
        (KeyCode::Char('f'), KeyModifiers::ALT, "tap:alt+f"),
        (KeyCode::Tab, KeyModifiers::NONE, "tap:tab"),
        (KeyCode::Char(','), KeyModifiers::NONE, "tap:comma"),
    ];
    for (code, m, want) in cases {
        let mut r = Recorder::default();
        r.key(&KeyEvent::new(code, m), std::time::Instant::now());
        let got = r.finish();
        assert_eq!(got, want);
        assert!(crate::protocol::parse_macro(&got).is_ok(), "{got} is not a valid step");
    }
}

#[test]
fn add_a_step_menu_covers_clicks_and_questions() {
    let mut app = macros_app();
    app.tabs[5].sel = 3; // [ Add a step... ]
    press(&mut app, KeyCode::Enter);
    let s = screen(&app);
    assert!(s.contains("add a step") && s.contains("Click left button") && s.contains("Type text...") && s.contains("Wait..."), "{s}");
    // a click is added at once
    for c in "click left".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.steps_now(), "click:left");
    // a question asks for the value
    app.tabs[5].sel = 3;
    press(&mut app, KeyCode::Enter);
    for c in "type".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(app.modal.step_prompt.is_some());
    assert!(screen(&app).contains("Text to type"));
    for c in "hello".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.steps_now(), "click:left,type:hello");
}

#[test]
fn wait_question_starts_at_100_and_bad_answers_are_refused() {
    let mut app = macros_app();
    app.tabs[5].rows[1].value = Value::Text("tap:a".into());
    app.tabs[5].sel = 3;
    press(&mut app, KeyCode::Enter);
    for c in "wait".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.modal.step_prompt.as_ref().unwrap().text, "100");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.steps_now(), "tap:a,wait:100");
    // a key the mouse cannot store is refused with a message
    app.modal.step_prompt = Some(StepPrompt { kind: StepKind::Tap, text: "ctrl+nosuchkey".into() });
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.steps_now(), "tap:a,wait:100");
    assert!(app.log.last().unwrap().starts_with("error"));
}

#[test]
fn undo_removes_the_last_step() {
    let mut app = macros_app();
    app.tabs[5].rows[1].value = Value::Text("tap:a,wait:50,tap:b".into());
    app.tabs[5].sel = 4; // [ Undo last step ]
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.steps_now(), "tap:a,wait:50");
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.steps_now(), "");
}

#[test]
fn a_macro_that_is_too_long_cannot_be_built() {
    let mut app = macros_app();
    let long = vec!["tap:a"; 40].join(","); // 80 events
    assert!(app.append_steps(&long));
    assert!(!app.append_steps(&long), "160 events do not fit in the mouse");
    assert!(app.log.last().unwrap().contains("at most 125"));
}

#[test]
fn growing_the_stage_count_gives_new_stages_default_colours() {
    assert_eq!(pad_colors("ff0000,00ff00"), "ff0000,00ff00,ffa000,ff00ff,00ffff,ff0000,ffff00,ffffff");
    assert_eq!(pad_colors("").split(',').count(), 8);
    assert_eq!(pad_colors("1,2,3,4,5,6,7,8,9").split(',').count(), 9, "longer lists are left alone");
}

#[test]
fn import_prompt_runs_profile_import_with_a_cleaned_path() {
    let mut app = App::for_test();
    app.cur = 7;
    press(&mut app, KeyCode::Char('i'));
    assert_eq!(app.modal.naming_kind, NameKind::ImportPath);
    for c in "\"/tmp/my profile.psf\"".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.sent(), vec![vec!["profile".to_string(), "import".into(), "/tmp/my profile.psf".into()]]);
    assert_eq!(clean_path("file:///a/b.psf"), "/a/b.psf");
}

#[test]
fn colour_previews_shrink_to_fit_the_smallest_terminal() {
    let all = pad_colors("");
    for selected in [false, true] {
        for chips in [all.as_str(), "ff0000"] {
            let row = text("Scroll-light colours", chips, 1);
            let state = RowState { selected, editing: false, dim: false, dirty: false, typed: None };
            let avail = 80usize.saturating_sub(4 + VALUE_X as usize);
            let line = row_line(&row, state, None, "", avail);
            assert!(line.width() <= 80 - 2, "{} cells for {chips}", line.width());
        }
    }
}

fn click_hint(app: &mut App, h: Hint) {
    let f = footer_rect(app.area);
    let mut x = 0;
    for k in app.footer_hints() {
        if k == h {
            click(app, f.x + x, f.y);
            return;
        }
        x += k.text().chars().count() as u16 + 3;
    }
    panic!("{h:?} is not shown on this page");
}

#[test]
fn the_key_hints_under_the_log_can_be_clicked() {
    let mut app = App::for_test();
    click_hint(&mut app, Hint::Help);
    assert!(app.modal.help);
    app.modal.help = false;
    let popup = app.live.popup.load(std::sync::atomic::Ordering::Relaxed);
    click_hint(&mut app, Hint::Popup);
    assert_ne!(app.live.popup.load(std::sync::atomic::Ordering::Relaxed), popup);
    click_hint(&mut app, Hint::Pages);
    assert_eq!(app.cur, 1);
    click_hint(&mut app, Hint::Undo);
    click_hint(&mut app, Hint::Quit);
    assert!(app.quit);
}

#[test]
fn the_function_list_closes_with_the_x_or_a_click_outside() {
    let mut app = App::for_test();
    app.cur = 4;
    app.open_chooser();
    let r = chooser_rect(app.area, app.chooser_total());
    click(&mut app, r.x + r.width - 3, r.y);
    assert!(app.modal.chooser.is_none());
    app.open_chooser();
    click(&mut app, 0, 0);
    assert!(app.modal.chooser.is_none());
    assert!(app.sent().is_empty());
}

#[test]
fn prompts_have_clickable_ok_and_cancel() {
    let mut app = App::for_test();
    app.cur = 7;
    app.start_naming(NameKind::SaveAs);
    app.modal.naming = Some("work".into());
    let r = prompt_rect(app.area);
    click(&mut app, r.x + 2, r.y + 3);
    assert!(app.modal.naming.is_none());
    assert_eq!(app.sent(), vec![vec!["profile".to_string(), "save".into(), "work".into()]]);
    app.start_naming(NameKind::SaveAs);
    click(&mut app, r.x + 1 + 6 + 2 + 1, r.y + 3);
    assert!(app.modal.naming.is_none());
    assert_eq!(app.sent().len(), 1);
}

#[test]
fn the_recording_box_can_be_finished_or_cancelled_with_the_mouse() {
    let mut app = macros_app();
    app.modal.recording = Some(Recorder::default());
    let r = recording_rect(app.area);
    let done = text::recording::DONE.chars().count() as u16;
    click(&mut app, r.x + 1 + done + 3, r.y + 6);
    assert!(app.modal.recording.is_none());
    app.modal.recording = Some(Recorder::default());
    click(&mut app, r.x + 2, r.y + 6);
    assert!(app.modal.recording.is_none());
}

#[test]
fn the_picker_has_keyboard_presets_and_closes_on_an_outside_click() {
    let mut app = App::for_test();
    app.cur = 3;
    app.tabs[3].sel = 1;
    app.open_picker();
    let before = app.modal.picker.as_ref().unwrap().colors[0];
    app.key(key(KeyCode::Char('p')));
    let after = app.modal.picker.as_ref().unwrap().colors[0];
    assert_ne!(before, after);
    assert!(PRESETS.contains(&after));
    app.key(key(KeyCode::Char('P')));
    assert!(PRESETS.contains(&app.modal.picker.as_ref().unwrap().colors[0]));
    click(&mut app, 0, 0);
    assert!(app.modal.picker.is_none());
}

#[test]
fn clicking_away_ends_custom_function_typing() {
    let mut app = App::for_test();
    app.cur = 4;
    app.modal.editing_custom = true;
    click(&mut app, 3, 4);
    assert!(!app.modal.editing_custom);
}

#[test]
fn the_language_key_and_hint_open_a_menu_that_starts_on_the_current_language() {
    let mut app = App::for_test();
    app.key(key(KeyCode::Char('L')));
    let c = app.modal.chooser.clone().unwrap();
    assert!(c.lang);
    assert_eq!(app.chooser_items().len(), crate::tui::i18n::Lang::ALL.len());
    assert_eq!(c.sel, 0);
    app.key(key(KeyCode::Esc));
    assert!(app.modal.chooser.is_none());
    click_hint(&mut app, Hint::Language);
    assert!(app.modal.chooser.is_some());
}

/// `e` starts typing into the selected numeric row.
fn type_digits(app: &mut App, digits: &str) {
    for c in digits.chars() {
        app.key(key(KeyCode::Char(c)));
    }
}

/// `e` starts typing into the selected numeric row.
fn type_number(app: &mut App, digits: &str) {
    app.key(key(KeyCode::Char('e')));
    type_digits(app, digits);
}

#[test]
fn numbers_can_be_typed_and_snap_to_the_rows_step_and_range() {
    let mut app = App::for_test();
    app.cur = 1;
    app.tabs[1].sel = 2; // stage 1 DPI: 50 to 26000, step 50
    type_number(&mut app, "820");
    assert_eq!(app.typing.as_deref(), Some("820"));
    app.key(key(KeyCode::Enter));
    assert_eq!(app.tabs[1].rows[2].value, Value::Num(800));
    assert!(app.typing.is_none());
    type_number(&mut app, "99999");
    app.key(key(KeyCode::Enter));
    assert_eq!(app.tabs[1].rows[2].value, Value::Num(26000));
    type_number(&mut app, "5");
    app.key(key(KeyCode::Esc));
    assert_eq!(app.tabs[1].rows[2].value, Value::Num(26000));
}

#[test]
fn typing_a_value_and_pressing_down_confirms_it_and_moves_on() {
    let mut app = App::for_test();
    app.cur = 1;
    app.tabs[1].sel = 2;
    type_number(&mut app, "1600");
    app.key(key(KeyCode::Down));
    assert_eq!(app.tabs[1].rows[2].value, Value::Num(1600));
    assert_eq!(app.tabs[1].sel, 3);
}

#[test]
fn typing_the_stage_count_rebuilds_the_stage_rows() {
    let mut app = App::for_test();
    app.cur = 1;
    app.tabs[1].sel = 0;
    type_number(&mut app, "3");
    app.key(key(KeyCode::Enter));
    let stages = app.tabs[1].rows.iter().filter(|r| r.label.starts_with("Stage ") && r.label.ends_with("DPI")).count();
    assert_eq!(stages, 3);
}

#[test]
fn a_polling_rate_can_be_typed_and_unknown_values_are_refused() {
    let mut app = App::for_test();
    app.cur = 2;
    type_number(&mut app, "250");
    app.key(key(KeyCode::Enter));
    assert!(matches!(&app.tabs[2].rows[0].value, Value::Choice(i) if RATES[*i] == "250"));
    type_number(&mut app, "300");
    app.key(key(KeyCode::Enter));
    assert!(matches!(&app.tabs[2].rows[0].value, Value::Choice(i) if RATES[*i] == "250"));
    assert!(app.log.last().unwrap().contains("not an option"));
}

#[test]
fn digits_always_switch_pages_even_on_rows_that_take_a_number() {
    let mut app = App::for_test();
    app.cur = 1;
    app.tabs[1].sel = 2; // a DPI value
    app.key(key(KeyCode::Char('3')));
    assert_eq!(app.cur, 2);
    assert!(app.typing.is_none());
    assert_eq!(app.tabs[1].rows[2].value, Value::Num(800));
}

#[test]
fn the_picker_takes_typed_channel_values() {
    let mut app = App::for_test();
    app.cur = 3;
    app.tabs[3].sel = 1;
    app.open_picker();
    type_digits(&mut app, "128");
    assert_eq!(app.modal.picker.as_ref().unwrap().colors[0][0], 128);
    type_digits(&mut app, "9"); // 1289 does not fit: start over
    assert_eq!(app.modal.picker.as_ref().unwrap().colors[0][0], 9);
    app.key(key(KeyCode::Backspace));
    assert_eq!(app.modal.picker.as_ref().unwrap().colors[0][0], 0);
    app.key(key(KeyCode::Down));
    type_digits(&mut app, "255");
    let c = app.modal.picker.as_ref().unwrap().colors[0];
    assert_eq!((c[0], c[1]), (0, 255));
}

#[test]
fn the_popup_hint_is_only_on_the_status_page_and_edit_appears_on_editable_rows() {
    let mut app = App::for_test();
    assert!(app.footer_hints().contains(&Hint::Popup));
    app.cur = 1;
    app.tabs[1].sel = 2; // a number
    let hints = app.footer_hints();
    assert!(!hints.contains(&Hint::Popup));
    assert!(hints.contains(&Hint::Edit));
    click_hint(&mut app, Hint::Edit);
    assert_eq!(app.typing.as_deref(), Some(""));
    app.key(key(KeyCode::Esc));
    app.tabs[1].sel = app.tabs[1].rows.iter().position(|r| matches!(r.value, Value::Action)).unwrap();
    assert!(!app.footer_hints().contains(&Hint::Edit));
}
