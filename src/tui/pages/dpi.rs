//! DPI page.

use crate::tui::*;

/// The DPI page: only the stages in use get a row; `vals` remembers the others.
pub(crate) fn dpi_rows(n: usize, active: i64, vals: &[i64; 8], colors: &str) -> Vec<Row> {
    let mut v = vec![num("Stages in use", n as i64, 1, 8, 1, 0), num("Starting stage", active.clamp(1, n as i64), 1, n as i64, 1, 0)];
    for (i, val) in vals.iter().enumerate().take(n) {
        v.push(num(&format!("Stage {} DPI", i + 1), *val, 50, 26000, 50, 0));
    }
    v.push(action("Apply", 0));
    v.push(text("Scroll-light colours", colors, 1));
    v.push(action("Apply", 1));
    v
}

pub(crate) fn stage_number(label: &str) -> Option<usize> {
    label.strip_prefix("Stage ")?.split(' ').next()?.parse().ok()
}

/// For a "Stage k DPI" row: the stage's scroll-light colour and a marker for the starting stage.
pub(crate) fn stage_decor(tab: &Tab, r: &Row) -> (Option<[u8; 3]>, String) {
    let k = match stage_number(&r.label) {
        Some(k) => k,
        None => return (None, String::new()),
    };
    let colors = tab
        .rows
        .iter()
        .find(|r| is_color_row(r))
        .and_then(|r| if let Value::Text(t) = &r.value { Some(parse_color_list(t)) } else { None })
        .unwrap_or_default();
    let chip = colors.get(k - 1).or(colors.last()).copied();
    let active = tab
        .rows
        .iter()
        .find(|r| r.label.starts_with("Starting stage"))
        .and_then(|r| if let Value::Num(a) = r.value { Some(a as usize) } else { None })
        .unwrap_or(1);
    let used = tab.rows.first().and_then(|r| if let Value::Num(n) = r.value { Some(n as usize) } else { None }).unwrap_or(1);
    (chip, if k == active.min(used) { "<- starts here".into() } else { String::new() })
}

/// Complete a colour list to eight entries with the default colours (one scroll-light colour per possible stage).
pub(crate) fn pad_colors(list: &str) -> String {
    let mut have: Vec<String> = list.split(',').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    for d in DEFAULT_COLORS.split(',').skip(have.len()) {
        have.push(d.to_string());
    }
    have.join(",")
}

impl App {
    /// The number of DPI stages changed: add or remove stage rows, keeping every value.
    pub(crate) fn rebuild_dpi(&mut self) {
        let t = &mut self.tabs[self.cur];
        for r in &t.rows {
            if let (Some(k), Value::Num(v)) = (stage_number(&r.label), &r.value) {
                self.dpi_mem[k - 1] = *v;
            }
        }
        let n = match t.rows[0].value {
            Value::Num(n) => (n as usize).clamp(1, 8),
            _ => 4,
        };
        let active = match t.rows[1].value {
            Value::Num(a) => a,
            _ => 1,
        };
        let colors =
            t.rows.iter().find(|r| is_color_row(r)).and_then(|r| if let Value::Text(c) = &r.value { Some(c.clone()) } else { None }).unwrap_or_default();
        let old: Vec<(Row, (Value, String))> = t.rows.iter().cloned().zip(t.applied.iter().cloned()).collect();
        t.rows = dpi_rows(n, active, &self.dpi_mem, &pad_colors(&colors));
        // rows that already existed keep their "last applied" value; new ones start clean
        t.applied = t
            .rows
            .iter()
            .map(|r| {
                if matches!(r.value, Value::Action) {
                    return (r.value.clone(), String::new());
                }
                old.iter().find(|(o, _)| o.label == r.label).map(|(_, a)| a.clone()).unwrap_or((r.value.clone(), r.extra.clone()))
            })
            .collect();
        // the stage count row itself is the one being edited: it differs from "applied" like any other change
        t.sel = t.sel.min(t.rows.len() - 1);
    }
}
