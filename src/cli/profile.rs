//! `profile save|load|current|list|show|rename|delete`.

use super::{execute, Context, ProfileCmd};
use crate::settings::profiles::summarize;
use crate::settings::store::replay_rank;
use std::io::Write;

fn w(e: std::io::Error) -> String {
    e.to_string()
}

pub fn run(cmd: ProfileCmd, ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    let store = &ctx.store;
    match cmd {
        ProfileCmd::Save { name } => {
            let name = match name {
                Some(n) => n,
                None => store.active_profile().ok_or("no active profile: give a name, e.g. `m7ctl profile save gaming`")?,
            };
            let n = store.save_profile(&name)?;
            writeln!(out, "profile '{name}' saved ({n} settings) to {} and is now the active profile", store.profile_path(&name)?.display()).map_err(w)
        }
        ProfileCmd::Load { name } => replay_profile(&name, ctx, out),
        ProfileCmd::Current => match store.active_profile() {
            Some(n) => {
                let note = if store.profile_modified(&n) == Some(true) {
                    " (the current settings differ from the saved profile)"
                } else {
                    " (matches the current settings)"
                };
                writeln!(out, "{n}{note}").map_err(w)
            }
            None => writeln!(out, "no active profile (`m7ctl profile save <name>` creates one)").map_err(w),
        },
        ProfileCmd::List => {
            let names = store.list_profiles();
            if names.is_empty() {
                writeln!(out, "no profiles yet (`m7ctl profile save <name>`)").map_err(w)?;
            }
            let active = store.active_profile();
            for n in names {
                let summary = store.read_profile(&n).map(|s| summarize(&s)).unwrap_or_default();
                writeln!(out, "{} {n:<20} {summary}", if active.as_deref() == Some(n.as_str()) { "*" } else { " " }).map_err(w)?;
            }
            Ok(())
        }
        ProfileCmd::Show { name } => {
            let st = match name {
                Some(name) => store.read_profile(&name)?,
                None => store.read_state(),
            };
            if st.is_empty() {
                writeln!(out, "(empty)").map_err(w)?;
            }
            for (k, v) in st {
                writeln!(out, "{k:<16} {}", v.join(" ")).map_err(w)?;
            }
            Ok(())
        }
        ProfileCmd::Import { file, name } => {
            let bytes = std::fs::read(&file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("imported").to_string();
            let imported = crate::settings::psf::import(&bytes, &stem, &store.dpi_table())?;
            let name = name.unwrap_or(imported.name);
            if store.profile_path(&name)?.exists() {
                return Err(format!("a profile named '{name}' already exists"));
            }
            store.write_profile(&name, &imported.state)?;
            writeln!(out, "profile '{name}' created from {} ({})", file.display(), summarize(&imported.state)).map_err(w)?;
            for n in &imported.notes {
                writeln!(out, "note: {n}").map_err(w)?;
            }
            writeln!(out, "send it to the mouse with: m7ctl profile load {name}").map_err(w)
        }
        ProfileCmd::Rename { old, new } => {
            store.rename_profile(&old, &new)?;
            writeln!(out, "profile '{old}' renamed to '{new}'").map_err(w)
        }
        ProfileCmd::Delete { name } => {
            store.delete_profile(&name)?;
            writeln!(out, "profile '{name}' deleted").map_err(w)
        }
    }
}

/// Write every setting of a profile to the mouse (macros first, buttons last) and make it the active profile.
pub fn replay_profile(name: &str, ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    let mut st = ctx.store.read_profile(name)?;
    st.sort_by_key(|(k, _)| replay_rank(k));
    ctx.store.write_state(&Vec::new())?;
    let n = st.len();
    for (i, (key, args)) in st.into_iter().enumerate() {
        let cmd = key.split(':').next().unwrap_or("").to_string();
        let mut argv = vec![cmd.clone()];
        argv.extend(args);
        writeln!(out, "[{}/{n}] {cmd} {}", i + 1, argv[1..].join(" ")).map_err(w)?;
        execute(&argv, ctx, out).map_err(|e| format!("profile '{name}' stopped at {key}: {e}"))?;
    }
    ctx.store.set_active(Some(name));
    writeln!(out, "profile '{name}' loaded and is now the active profile").map_err(w)
}
