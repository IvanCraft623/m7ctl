//! Status page, DPI-button watcher and popup toggle.

use crate::tui::*;

/// A 20-cell battery gauge with the percentage written inside it.
pub(crate) fn battery_bar(pct: u16) -> Vec<Span<'static>> {
    let colour = if pct > 50 {
        Color::Green
    } else if pct > 20 {
        Color::Yellow
    } else {
        Color::Red
    };
    let filled = (pct.min(100) as usize * 20 + 50) / 100;
    let text: Vec<char> = format!("{pct}%").chars().collect();
    let start = (20 - text.len()) / 2;
    (0..20)
        .map(|i| {
            let ch = if i >= start && i < start + text.len() { text[i - start] } else { ' ' };
            let (bg, fg) = if i < filled { (colour, Color::Black) } else { (Color::Rgb(60, 60, 60), Color::White) };
            Span::styled(ch.to_string(), Style::new().bg(bg).fg(fg).add_modifier(Modifier::BOLD))
        })
        .collect()
}

/// The main page: the device, then the settings m7ctl last wrote.
pub(crate) fn status_page(app: &App) -> Vec<Line<'static>> {
    let st = &app.cur_state;
    let get = |k: &str| st.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
    let colors = get("dpi-colors").map(|v| parse_color_list(v.first().map(String::as_str).unwrap_or(""))).unwrap_or_default();
    let active = get("dpi").and_then(|v| v.iter().position(|a| a == "--active").and_then(|i| v.get(i + 1)).and_then(|x| x.parse::<usize>().ok())).unwrap_or(1);
    let label = |k: &str| Span::styled(format!("{:<10}", tr(k)), Style::new().fg(Color::Cyan));
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines: Vec<Line<'static>> = vec![];
    let s = &app.live.status;
    if app.live.conn.as_deref() == Some("") {
        lines.push(Line::from(Span::styled(text::status::NOT_CONNECTED, Style::new().fg(Color::Red))));
    } else if let Some(e) = &s.error {
        lines.push(Line::from(Span::styled(format!("status failed: {e}"), Style::new().fg(Color::Red))));
    } else if s.model.is_empty() {
        lines.push(Line::from(Span::styled("reading the mouse...  (Enter to retry)", dim)));
    } else {
        lines.push(Line::from(vec![label(text::status::DEVICE), Span::raw(format!("{} ({})", s.device, tr(&s.link)))]));
        let mut bar = vec![label(text::status::BATTERY)];
        bar.extend(battery_bar(s.battery.parse().unwrap_or(0)));
        bar.push(Span::raw(format!("  {}", tr(&s.charge))));
        lines.push(Line::from(bar));
        let (txt, c) = if s.online == "true" { ("yes", Color::Green) } else { ("no (asleep or out of range)", Color::Red) };
        lines.push(Line::from(vec![label(text::status::ONLINE), Span::styled(txt, Style::new().fg(c))]));
    }
    lines.push(Line::from(""));
    if st.is_empty() {
        lines.push(Line::from(Span::styled("No settings written yet.", dim)));
    } else {
        if let Some(v) = get("dpi") {
            let mut sp = vec![label(text::status::DPI)];
            for (i, stage) in v.first().map(|x| x.split(',').map(String::from).collect::<Vec<_>>()).unwrap_or_default().into_iter().enumerate() {
                let bold =
                    if i + 1 == app.live.live_stage.map(usize::from).unwrap_or(active) { Style::new().add_modifier(Modifier::BOLD) } else { Style::new() };
                sp.push(Span::styled(stage, bold));
                if let Some(c) = colors.get(i).or(colors.last()) {
                    sp.push(Span::raw(" "));
                    sp.push(Span::styled("  ", Style::new().bg(rgb_color(*c))));
                }
                sp.push(Span::raw("   "));
            }
            lines.push(Line::from(sp));
        }
        if let Some(v) = get("rate") {
            lines.push(Line::from(vec![label(text::status::POLLING), Span::raw(format!("{} Hz", v.first().cloned().unwrap_or_default()))]));
        }
        if let Some(v) = get("light") {
            let mut sp = vec![label(text::status::LIGHTS), Span::raw(v.first().cloned().unwrap_or_default()), Span::raw("  ")];
            if v.first().map(String::as_str) != Some("off") {
                for c in v.get(1).filter(|c| colors_valid(c)).map(|c| parse_color_list(c)).unwrap_or_default() {
                    sp.push(Span::styled("    ", Style::new().bg(rgb_color(c))));
                    sp.push(Span::raw(" "));
                }
            }
            lines.push(Line::from(sp));
        }
    }
    if let Some(a) = &app.prof.active {
        lines.push(Line::from(vec![
            label(text::status::PROFILE),
            Span::raw(a.clone()),
            Span::styled(if app.prof.modified { text::status::UNSAVED } else { "" }, Style::new().fg(Color::Yellow)),
        ]));
    }
    let on = app.live.popup.load(std::sync::atomic::Ordering::Relaxed);
    lines.push(Line::from(vec![
        label(text::status::POPUP),
        Span::styled(if on { "on" } else { "off" }, Style::new().fg(if on { Color::Green } else { Color::DarkGray })),
        Span::styled("   DPI change, p toggles", dim),
    ]));
    lines
}

impl App {
    pub(crate) fn refresh_status(&mut self) {
        self.read_status(false);
    }

    /// Background refresh: a failed read (asleep mouse) keeps the last good values on screen.
    pub(crate) fn poll_status(&mut self) {
        self.read_status(true);
    }

    fn read_status(&mut self, quiet: bool) {
        if self.bg.busy {
            return;
        }
        self.bg.busy = true;
        let (tx, ctx) = (self.bg.tx.clone(), self.ctx.clone());
        self.run_job(move || {
            let info = match crate::cli::read_status(&ctx) {
                Ok(r) => StatusInfo {
                    device: r.device,
                    link: r.link.to_string(),
                    model: r.model,
                    battery: r.battery.to_string(),
                    charge: r.charge.to_string(),
                    online: r.online.to_string(),
                    error: None,
                },
                Err(e) => StatusInfo { error: Some(e), ..Default::default() },
            };
            if !(quiet && info.error.is_some()) {
                let _ = tx.send(Msg::Status(info));
            }
            let _ = tx.send(Msg::Done);
        });
    }

    /// Watch the connection (wired, wireless receiver, unplugged) and follow the DPI-stage reports of
    /// every interface found, so switching cables or sleeping receivers need no restart.
    pub(crate) fn spawn_dpi_watcher(&self) {
        use std::collections::HashSet;
        use std::sync::{Arc, Mutex};
        let (tx, device, popup, store) = (self.bg.tx.clone(), self.ctx.device.clone(), self.live.popup.clone(), self.ctx.store.clone());
        let busy: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        std::thread::spawn(move || {
            let mut last: Option<String> = None;
            loop {
                let found: Vec<(String, u16)> = match &device {
                    Some(d) => {
                        if crate::device::all_interfaces().is_ok_and(|all| all.iter().any(|i| &i.path == d)) {
                            vec![(d.clone(), 0)]
                        } else {
                            vec![]
                        }
                    }
                    None => crate::device::find_devices().unwrap_or_default().into_iter().map(|c| (c.path, c.pid)).collect(),
                };
                let link = match found.first() {
                    None => String::new(),
                    Some((_, crate::device::PID_WIRED)) => "wired".to_string(),
                    Some((_, 0)) => "device".to_string(),
                    Some(_) => "wireless".to_string(),
                };
                if last.as_ref() != Some(&link) {
                    last = Some(link.clone());
                    if tx.send(Msg::Link(link)).is_err() {
                        return;
                    }
                }
                for (path, _) in found {
                    if !busy.lock().unwrap().insert(path.clone()) {
                        continue;
                    }
                    let (tx, popup, busy, store) = (tx.clone(), popup.clone(), busy.clone(), store.clone());
                    std::thread::spawn(move || {
                        if let Ok(mut dev) = crate::device::Mouse::open(Some(&path)) {
                            while let Ok(r) = dev.read_report() {
                                if let Some((stage, dpi)) = crate::protocol::parse_dpi_event(&r, &store.dpi_table()) {
                                    if popup.load(std::sync::atomic::Ordering::Relaxed) {
                                        crate::notify::dpi_changed(stage, dpi);
                                    }
                                    if tx.send(Msg::Dpi(stage, dpi)).is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        busy.lock().unwrap().remove(&path);
                    });
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }

    pub(crate) fn choose_language(&mut self, l: i18n::Lang) {
        i18n::set_language(l);
        self.ctx.store.set_language(l.code());
        self.log.push(format!("language: {}", l.name()));
    }

    pub(crate) fn toggle_popup(&mut self) {
        let on = !self.live.popup.load(std::sync::atomic::Ordering::Relaxed);
        self.live.popup.store(on, std::sync::atomic::Ordering::Relaxed);
        self.ctx.store.set_popup(on);
        self.log.push(format!("DPI popup {}", if on { "on" } else { "off" }));
    }
}
