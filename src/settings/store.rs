//! The record of what m7ctl last wrote to the mouse (the mouse cannot be read back), plus the small
//! preference files that live next to it.
//!
//! Everything goes through a [`Store`] that owns its directory, so tests use a temporary one and
//! never touch the real configuration.

use std::path::{Path, PathBuf};

/// One line per setting: `key<TAB>arg<TAB>arg...`; the arguments are exactly what the CLI takes.
pub type State = Vec<(String, Vec<String>)>;

#[derive(Clone, Debug)]
pub struct Store {
    pub(super) dir: PathBuf,
}

impl Store {
    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    /// `$M7CTL_CONFIG_DIR` if set, else the platform's config directory plus `m7ctl`
    /// (`~/.config` or `$XDG_CONFIG_HOME` on Linux, `%APPDATA%` on Windows, `~/Library/Application Support` on macOS).
    pub fn default_location() -> Result<Store, String> {
        match std::env::var_os("M7CTL_CONFIG_DIR") {
            Some(v) if !v.is_empty() => Ok(Store::at(v)),
            _ => Ok(Store::at(dirs::config_dir().ok_or("cannot find the configuration directory")?.join("m7ctl"))),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn ensure_dir(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot create {}: {e}", self.dir.display()))
    }

    pub fn read_state(&self) -> State {
        std::fs::read_to_string(self.dir.join("state")).map(|t| parse_state(&t)).unwrap_or_default()
    }

    pub fn write_state(&self, st: &State) -> Result<(), String> {
        self.ensure_dir()?;
        std::fs::write(self.dir.join("state"), render_state(st)).map_err(|e| format!("cannot write the state record: {e}"))
    }

    /// Remember a setting (replacing the previous value of the same key).
    pub fn record(&self, key: &str, args: Vec<String>) -> Result<(), String> {
        let mut st = self.read_state();
        st.retain(|(k, _)| k != key);
        st.push((key.to_string(), args));
        self.write_state(&st)
    }

    pub fn forget(&self, key: &str) -> Result<(), String> {
        let mut st = self.read_state();
        st.retain(|(k, _)| k != key);
        self.write_state(&st)
    }

    /// A boolean preference stored as a file containing `on` or `off`.
    pub fn flag(&self, name: &str, default: bool) -> bool {
        std::fs::read_to_string(self.dir.join(name)).map(|t| t.trim() != "off").unwrap_or(default)
    }

    pub fn set_flag(&self, name: &str, on: bool) {
        if self.ensure_dir().is_ok() {
            let _ = std::fs::write(self.dir.join(name), if on { "on" } else { "off" });
        }
    }

    /// The sensor variant byte last seen in a status reply of a known model (the receiver does not report it).
    pub fn variant(&self) -> Option<u8> {
        std::fs::read_to_string(self.dir.join("variant")).ok().and_then(|t| u8::from_str_radix(t.trim().trim_start_matches("0x"), 16).ok())
    }

    pub fn set_variant(&self, variant: u8) {
        if self.variant() != Some(variant) && self.ensure_dir().is_ok() {
            let _ = std::fs::write(self.dir.join("variant"), format!("{variant:02x}"));
        }
    }

    /// The DPI table to use: the remembered variant's, or the M7 Pro's.
    pub fn dpi_table(&self) -> crate::protocol::DpiTable {
        self.variant().and_then(crate::protocol::DpiTable::for_variant).unwrap_or_else(crate::protocol::DpiTable::standard)
    }

    /// Interface language remembered by the terminal UI (`en`, `es`).
    pub fn language(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("lang")).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    }

    pub fn set_language(&self, code: &str) {
        if self.ensure_dir().is_ok() {
            let _ = std::fs::write(self.dir.join("lang"), code);
        }
    }

    /// Copy changes applied from the terminal UI into the active profile (default: yes).
    pub fn autosave(&self) -> bool {
        self.flag("autosave", true)
    }

    pub fn set_autosave(&self, on: bool) {
        self.set_flag("autosave", on)
    }

    /// Desktop popup when the DPI stage changes (default: yes).
    pub fn popup(&self) -> bool {
        self.flag("popup", true)
    }

    pub fn set_popup(&self, on: bool) {
        self.set_flag("popup", on)
    }
}

pub fn parse_state(text: &str) -> State {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut it = l.split('\t').map(String::from);
            (it.next().unwrap_or_default(), it.collect())
        })
        .collect()
}

pub fn render_state(st: &State) -> String {
    st.iter().map(|(k, v)| format!("{k}\t{}\n", v.join("\t"))).collect()
}

/// Replay order: macros first (buttons refer to them), buttons last.
pub fn replay_rank(key: &str) -> u8 {
    match key.split(':').next().unwrap_or("") {
        "macro" => 0,
        "dpi" => 1,
        "dpi-colors" => 2,
        "rate" => 3,
        "light" => 4,
        "lod" => 5,
        "power" => 6,
        "motion-wake" => 7,
        "button" => 8,
        _ => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip() {
        let st: State = vec![("dpi".into(), vec!["800,1600".into(), "--active".into(), "2".into()]), ("rate".into(), vec!["500".into()])];
        assert_eq!(parse_state(&render_state(&st)), st);
    }

    #[test]
    fn replay_puts_macros_first_and_buttons_last() {
        let mut keys = vec!["button:dpi", "rate", "macro:3", "dpi"];
        keys.sort_by_key(|k| replay_rank(k));
        assert_eq!(keys, vec!["macro:3", "dpi", "rate", "button:dpi"]);
    }

    #[test]
    fn record_and_forget_use_the_given_directory() {
        let dir = std::env::temp_dir().join(format!("m7ctl-store-{}", std::process::id()));
        let store = Store::at(&dir);
        store.record("rate", vec!["500".into()]).unwrap();
        store.record("rate", vec!["1000".into()]).unwrap();
        assert_eq!(store.read_state(), vec![("rate".to_string(), vec!["1000".to_string()])]);
        store.forget("rate").unwrap();
        assert!(store.read_state().is_empty());
        assert!(store.autosave() && store.popup());
        store.set_popup(false);
        assert!(!store.popup());
        let _ = std::fs::remove_dir_all(dir);
    }
}
