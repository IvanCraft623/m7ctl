//! Every fixed piece of user-facing text of the dialogs and chrome, in one place.
//! (Page rows and log messages are built next to the code that knows their values; a translation layer
//! would start from this file.)

pub(crate) mod tabs {
    pub(crate) const STATUS: &str = "Status";
    pub(crate) const DPI: &str = "DPI";
    pub(crate) const RATE: &str = "Rate";
    pub(crate) const LIGHTS: &str = "Lights";
    pub(crate) const BUTTONS: &str = "Buttons";
    pub(crate) const MACROS: &str = "Macros";
    pub(crate) const POWER: &str = "Power";
    pub(crate) const PROFILES: &str = "Profiles";
}

pub(crate) mod help {
    pub(crate) const TITLE: &str = " help ";
    pub(crate) const KEYS: &str = "Keys";
    pub(crate) const KEY_LINES: [&str; 6] = [
        "  L             language menu (English / Español)",
        "  Tab, 1-8      page              Up Down Home End   select",
        "  Left Right    change (PgUp PgDn: bigger steps)",
        "  Enter or a    apply (Buttons: Enter opens a list)   u   undo",
        "  e             type number / hex / function       q   quit",
        "  p             popup when the DPI stage changes",
    ];
    pub(crate) const PROFILES: &str = "Profiles page";
    pub(crate) const PROFILE_LINES: [&str; 3] = [
        "  l load   s save   n new   r rename   d delete   A auto-save",
        "  i import a vendor .psf file",
        "  Macros page: Record keys, then F2 done / F3 cancel",
    ];
    pub(crate) const MOUSE: &str = "Mouse";
    pub(crate) const MOUSE_LINE: &str = "  click, wheel, double-click = apply, click a colour to pick it";
    pub(crate) const MARKERS: &str = "Markers";
    pub(crate) const MARKER_STAR: &str = "  *  ";
    pub(crate) const MARKER_TEXT: &str = "changed, not applied yet     <- starts here";
    pub(crate) const CLOSE: &str = "any key closes";
}

pub(crate) mod quit {
    pub(crate) const TITLE: &str = " quit? ";
    pub(crate) const QUIT: &str = "[ Quit (y) ]";
    pub(crate) const SAVE_AND_QUIT: &str = "[ Save and quit (s) ]";
    pub(crate) const STAY: &str = "[ Stay ]";
    pub(crate) fn not_applied(pages: &str) -> String {
        format!("Not applied to the mouse yet: {pages}")
    }
    pub(crate) fn unsaved_profile(name: &str) -> String {
        format!("Profile '{name}' has unsaved changes")
    }
}

pub(crate) mod profiles {
    pub(crate) const LOAD: &str = "[ Load ]";
    pub(crate) const SAVE: &str = "[ Save ]";
    pub(crate) const SAVE_AS: &str = "[ Save as new... ]";
    pub(crate) const RENAME: &str = "[ Rename... ]";
    pub(crate) const DELETE: &str = "[ Delete ]";
    pub(crate) const CONFIRM_DELETE: &str = "[ Click again to delete ]";
    pub(crate) fn save_into(name: &str) -> String {
        format!("[ Save into '{name}' ]")
    }
}

pub(crate) mod recording {
    pub(crate) const TITLE: &str = " recording ";
    pub(crate) const PROMPT: &str = "Press the keys the macro should send.";
    pub(crate) const PAUSES: &str = "Pauses longer than 0.4 s are recorded too.";
    pub(crate) const SO_FAR: &str = "So far: ";
    pub(crate) const NOTHING: &str = "nothing yet";
    pub(crate) const DONE: &str = "[ F2: done ]";
    pub(crate) const CANCEL: &str = "[ F3: cancel ]";
}

pub(crate) mod status {
    pub(crate) const DEVICE: &str = "Device";
    pub(crate) const BATTERY: &str = "Battery";
    pub(crate) const ONLINE: &str = "Online";
    pub(crate) const DPI: &str = "DPI";
    pub(crate) const POLLING: &str = "Polling";
    pub(crate) const LIGHTS: &str = "Lights";
    pub(crate) const PROFILE: &str = "Profile";
    pub(crate) const POPUP: &str = "Popup";
    pub(crate) const UNSAVED: &str = "  (unsaved)";
    pub(crate) const NOT_CONNECTED: &str = "Mouse not connected  (plug in the cable or the receiver)";
}
