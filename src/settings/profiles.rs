//! Named copies of the settings record, stored as `profiles/<name>.profile` next to the state file.

use super::store::{parse_state, render_state, State, Store};
use std::path::PathBuf;

pub fn valid_profile_name(n: &str) -> Result<(), String> {
    if n.is_empty() || n.len() > 40 || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("profile names use letters, digits, - and _ (at most 40 characters)".into());
    }
    Ok(())
}

impl Store {
    pub fn profile_path(&self, name: &str) -> Result<PathBuf, String> {
        valid_profile_name(name)?;
        Ok(self.dir.join("profiles").join(format!("{name}.profile")))
    }

    fn active_file(&self) -> PathBuf {
        self.dir.join("active")
    }

    /// Name written in the `active` file, even if its profile no longer exists.
    fn active_name_raw(&self) -> Option<String> {
        std::fs::read_to_string(self.active_file()).ok().map(|t| t.trim().to_string())
    }

    /// The profile treated as "the one in use" (set by `profile load` / `profile save`).
    pub fn active_profile(&self) -> Option<String> {
        let name = self.active_name_raw()?;
        if valid_profile_name(&name).is_ok() && self.profile_path(&name).map(|p| p.exists()).unwrap_or(false) {
            Some(name)
        } else {
            None
        }
    }

    pub fn set_active(&self, name: Option<&str>) {
        let _ = std::fs::create_dir_all(&self.dir);
        match name {
            Some(n) => {
                let _ = std::fs::write(self.active_file(), n);
            }
            None => {
                let _ = std::fs::remove_file(self.active_file());
            }
        }
    }

    pub fn read_profile(&self, name: &str) -> Result<State, String> {
        std::fs::read_to_string(self.profile_path(name)?).map(|t| parse_state(&t)).map_err(|_| format!("no profile named '{name}' (see `m7ctl profile list`)"))
    }

    /// `Some(true)` when the profile file differs from the current settings record.
    pub fn profile_modified(&self, name: &str) -> Option<bool> {
        self.read_profile(name).ok().map(|p| !same_settings(&p, &self.read_state()))
    }

    /// Store the current settings record as profile `name` and make it the active one. Returns the setting count.
    pub fn save_profile(&self, name: &str) -> Result<usize, String> {
        let st = self.read_state();
        if st.is_empty() {
            return Err("nothing to save yet: m7ctl has not written any setting to the mouse".into());
        }
        let path = self.profile_path(name)?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, render_state(&st)).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        self.set_active(Some(name));
        Ok(st.len())
    }

    /// Store `state` as profile `name` without touching the active profile.
    pub fn write_profile(&self, name: &str, state: &State) -> Result<(), String> {
        let path = self.profile_path(name)?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, render_state(state)).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    pub fn list_profiles(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir.join("profiles"))
            .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".profile")).map(String::from)).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    pub fn rename_profile(&self, old: &str, new: &str) -> Result<(), String> {
        let (from, to) = (self.profile_path(old)?, self.profile_path(new)?);
        if !from.exists() {
            return Err(format!("no profile named '{old}'"));
        }
        if to.exists() {
            return Err(format!("a profile named '{new}' already exists"));
        }
        std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
        if self.active_name_raw().as_deref() == Some(old) {
            self.set_active(Some(new));
        }
        Ok(())
    }

    pub fn delete_profile(&self, name: &str) -> Result<(), String> {
        let path = self.profile_path(name)?;
        std::fs::remove_file(&path).map_err(|_| format!("no profile named '{name}'"))?;
        if self.active_name_raw().as_deref() == Some(name) {
            self.set_active(None);
        }
        Ok(())
    }
}

pub fn same_settings(a: &State, b: &State) -> bool {
    let norm = |s: &State| {
        let mut v: State = s.clone();
        v.sort();
        v
    };
    norm(a) == norm(b)
}

/// A short human summary of a settings record, for lists.
pub fn summarize(st: &State) -> String {
    let get = |k: &str| st.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
    let mut parts = vec![];
    if let Some(v) = get("dpi") {
        let stages: Vec<&str> = v.first().map(|x| x.split(',').collect()).unwrap_or_default();
        if let (Some(lo), Some(hi)) = (stages.first(), stages.last()) {
            parts.push(if stages.len() > 1 { format!("{lo}-{hi} DPI ({} stages)", stages.len()) } else { format!("{lo} DPI") });
        }
    }
    if let Some(v) = get("rate") {
        parts.push(format!("{} Hz", v.first().cloned().unwrap_or_default()));
    }
    if let Some(v) = get("light") {
        parts.push(format!("lights {}", v.first().cloned().unwrap_or_default()));
    }
    let buttons = st.iter().filter(|(k, _)| k.starts_with("button:")).count();
    if buttons > 0 {
        parts.push(format!("{buttons} button{}", if buttons == 1 { "" } else { "s" }));
    }
    let macros = st.iter().filter(|(k, _)| k.starts_with("macro:")).count();
    if macros > 0 {
        parts.push(format!("{macros} macro{}", if macros == 1 { "" } else { "s" }));
    }
    if parts.is_empty() {
        "(empty)".into()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> Store {
        Store::at(std::env::temp_dir().join(format!("m7ctl-prof-{tag}-{}", std::process::id())))
    }

    #[test]
    fn profile_names_are_safe() {
        assert!(valid_profile_name("gaming-2").is_ok());
        assert!(valid_profile_name("../evil").is_err());
        assert!(valid_profile_name("").is_err());
    }

    #[test]
    fn same_settings_ignores_order() {
        let a: State = vec![("a".into(), vec![]), ("b".into(), vec!["1".into()])];
        let b: State = vec![("b".into(), vec!["1".into()]), ("a".into(), vec![])];
        assert!(same_settings(&a, &b));
    }

    #[test]
    fn summaries_are_readable() {
        let st: State = vec![("dpi".into(), vec!["800,1600,3200".into()]), ("rate".into(), vec!["1000".into()]), ("button:dpi".into(), vec![])];
        assert_eq!(summarize(&st), "800-3200 DPI (3 stages), 1000 Hz, 1 button");
        assert_eq!(summarize(&vec![]), "(empty)");
    }

    #[test]
    fn save_rename_delete_follow_the_active_profile() {
        let s = temp_store("lifecycle");
        s.record("rate", vec!["500".into()]).unwrap();
        s.save_profile("one").unwrap();
        assert_eq!(s.active_profile().as_deref(), Some("one"));
        assert_eq!(s.profile_modified("one"), Some(false));
        s.rename_profile("one", "two").unwrap();
        assert_eq!(s.active_profile().as_deref(), Some("two"));
        s.record("rate", vec!["1000".into()]).unwrap();
        assert_eq!(s.profile_modified("two"), Some(true));
        s.delete_profile("two").unwrap();
        assert!(s.active_profile().is_none() && s.list_profiles().is_empty());
        let _ = std::fs::remove_dir_all(s.dir());
    }
}
