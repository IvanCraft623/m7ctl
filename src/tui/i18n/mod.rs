//! Translation of the terminal UI. The code writes English; everything shown goes through [`tr`], which looks the
//! text up in the catalog of the chosen language and falls back to the English text.
//!
//! Catalog keys are the English texts as they appear on screen. A key may contain `{}` placeholders, so
//! "macro 3 stored (12 events)" is found by the key "macro {} stored ({} events)". Commands, option names and
//! key names stay English everywhere.
//!
//! English is the default. Another language is chosen with `M7CTL_LANG=es` or with the `L` key in the interface
//! (remembered in the configuration directory); the system locale is deliberately not used.
//! To add a language: add a catalog file next to `es.rs`, a variant of [`Lang`] and a line in [`catalog`].

mod es;

use ratatui::text::{Line, Span};
use std::borrow::Cow;
use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Lang {
    En,
    Es,
}

static CHOSEN: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// Per-thread override, so tests can use another language without affecting each other.
    static OVERRIDE: Cell<Option<Lang>> = const { Cell::new(None) };
}

fn parse(code: &str) -> Option<Lang> {
    let c = code.to_ascii_lowercase();
    if c.starts_with("es") {
        Some(Lang::Es)
    } else if c.starts_with("en") || c == "c" || c == "posix" {
        Some(Lang::En)
    } else {
        None
    }
}

/// Pick the language at start: `M7CTL_LANG`, else the one remembered in the configuration, else English.
pub(crate) fn init(remembered: Option<&str>) {
    let from_env = std::env::var("M7CTL_LANG").ok().and_then(|v| parse(&v));
    set_language(from_env.or_else(|| remembered.and_then(parse)).unwrap_or(Lang::En));
}

pub(crate) fn set_language(l: Lang) {
    CHOSEN.store(if l == Lang::Es { 2 } else { 1 }, Ordering::Relaxed);
}

pub(crate) fn lang() -> Lang {
    OVERRIDE.with(|o| o.get()).unwrap_or(if CHOSEN.load(Ordering::Relaxed) == 2 { Lang::Es } else { Lang::En })
}

impl Lang {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Es => "Español",
        }
    }

    /// Every language, in the order the language menu lists them.
    pub(crate) const ALL: [Lang; 2] = [Lang::En, Lang::Es];
}

fn catalog(l: Lang) -> &'static [(&'static str, &'static str)] {
    match l {
        Lang::En => &[],
        Lang::Es => es::CATALOG,
    }
}

/// Match `text` against a key with `{}` placeholders and return what the placeholders captured.
fn captures<'a>(key: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = key.split("{}").collect();
    let mut rest = text.strip_prefix(parts[0])?;
    let mut caps = vec![];
    for (i, part) in parts.iter().enumerate().skip(1) {
        let last = i == parts.len() - 1;
        let at = if last { rest.strip_suffix(part).map(|r| r.len()) } else { rest.find(part) }?;
        caps.push(&rest[..at]);
        rest = &rest[at + part.len()..];
    }
    rest.is_empty().then_some(caps)
}

fn fill(template: &str, caps: &[&str]) -> String {
    let mut out = String::new();
    for (i, part) in template.split("{}").enumerate() {
        if i > 0 {
            out.push_str(caps.get(i - 1).copied().unwrap_or(""));
        }
        out.push_str(part);
    }
    out
}

/// The translation of `text`, if the catalog has one.
fn translate(text: &str) -> Option<String> {
    let cat = catalog(lang());
    if cat.is_empty() || text.trim().is_empty() {
        return None;
    }
    if let Some((_, to)) = cat.iter().find(|(k, _)| *k == text) {
        return Some(to.to_string());
    }
    cat.iter().filter(|(k, _)| k.contains("{}")).find_map(|(key, to)| captures(key, text).map(|caps| fill(to, &caps)))
}

/// The text in the chosen language (the text itself when there is no translation).
pub(crate) fn tr(text: &str) -> Cow<'_, str> {
    match translate(text) {
        Some(t) => Cow::Owned(t),
        None => Cow::Borrowed(text),
    }
}

/// Like [`tr`] for a text whose meaning depends on where it appears: the catalog key is `context|text`.
pub(crate) fn tr_ctx<'a>(context: &str, text: &'a str) -> Cow<'a, str> {
    match translate(&format!("{context}|{text}")) {
        Some(t) => Cow::Owned(t),
        None => Cow::Borrowed(text),
    }
}

/// Translate every span of already built lines.
pub(crate) fn localize(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    if lang() == Lang::En {
        return lines;
    }
    lines
        .into_iter()
        .map(|mut line| {
            line.spans = line.spans.into_iter().map(|s| Span::styled(tr(&s.content).into_owned(), s.style)).collect();
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes() {
        assert_eq!(parse("es_ES.UTF-8"), Some(Lang::Es));
        assert_eq!(parse("en_US"), Some(Lang::En));
        assert_eq!(parse("fr_FR"), None);
    }

    #[test]
    fn templates_capture_and_refill() {
        assert_eq!(captures("macro {} stored ({} events)", "macro 3 stored (12 events)"), Some(vec!["3", "12"]));
        assert_eq!(captures("macro {} stored ({} events)", "macro 3 cleared"), None);
        assert_eq!(fill("macro {} guardada ({} eventos)", &["3", "12"]), "macro 3 guardada (12 eventos)");
    }
}
