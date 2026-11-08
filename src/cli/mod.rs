//! Command-line interface. [`Cli`] describes every command (clap generates the help, the shell
//! completions and the man page from it); [`execute`] runs one command line against a [`Context`].

mod apply;
mod info;
mod profile;

use crate::device::Mouse;
use crate::settings::Store;
use clap::{Args, CommandFactory, Parser, Subcommand};
use std::io::Write;
use std::sync::Arc;

pub use info::{read_status, StatusReport};
pub use profile::replay_profile;

const AFTER_HELP: &str = "\
Settings are stored in the mouse, which cannot report them back: m7ctl records what it last wrote, \
and profiles are copies of that record.";

const BUTTON_HELP: &str = "\
Buttons: left right middle back forward dpi.
Functions: left right middle back forward, dpi-loop dpi-plus dpi-minus, play-pause mute volume-up \
volume-down prev-track next-track stop media-select mail calculator my-computer, copy paste cut \
select-all new save print open undo, close-window display-desktop run minimize maximize, disable \
three-click tilt-left tilt-right switch-effect switch-polling lights-toggle, fire:<interval>[,<loop>] \
(interval 5-255), macro:<slot>[,<mode 0-2>[,<times>]], dpi-lock:<1-11> (DPI forced to preset N while \
held), key:<combo> (e.g. key:ctrl+c key:alt+f4 key:win+d).";

#[derive(Parser, Debug)]
#[command(name = "m7ctl", version, about = "Configure a Machenike M7 Pro mouse (wired or wireless receiver)", after_help = AFTER_HELP)]
pub struct Cli {
    /// Use this HID device path instead of detecting the mouse (see `list`)
    #[arg(short, long, global = true, value_name = "PATH")]
    pub device: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show the mouse's configuration interface
    List {
        /// Show every HID interface of the mouse (useful for bug reports)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Model, battery and charge state
    Status {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Set up to 8 DPI stages (multiples of 50, up to 26000)
    Dpi {
        /// Comma-separated DPI values, e.g. 800,1600,3200
        values: String,
        /// Stage the mouse switches to (1-based)
        #[arg(long, default_value_t = 1)]
        active: usize,
    },
    /// Polling rate in Hz (125, 250, 500 or 1000)
    Rate { hz: u32 },
    /// Give a button a function
    #[command(after_help = BUTTON_HELP)]
    Button {
        /// left, right, middle, back, forward or dpi
        button: String,
        /// What the button should do (see below)
        function: String,
        /// Allow taking the left click away from the left button
        #[arg(long)]
        force: bool,
    },
    /// Main lights
    #[command(subcommand)]
    Light(LightCmd),
    /// Colours of the scroll lights, one per DPI stage (1 to 8, rrggbb,rrggbb,...)
    DpiColors { colors: String },
    /// Lift-off distance in mm (1 or 2)
    Lod { mm: u8 },
    /// Power-down time in minutes (3-10) and the lights-off-while-moving flag
    Power {
        minutes: u8,
        /// Switch the lights off while the mouse moves
        #[arg(value_parser = ["lights-off-moving"])]
        lights_off_moving: Option<String>,
        /// Allow minutes outside 3-10
        #[arg(long)]
        force: bool,
    },
    /// Wake the mouse on motion
    MotionWake {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// Store a macro in slot 1-15, then give a button `macro:<slot>`
    Macro {
        /// Slot 1-15
        slot: u8,
        /// Steps like tap:ctrl+c, type:hello, down:a, up:a, wait:100 - or `clear` to stop the macro
        steps: String,
    },
    /// Upload a macro from raw events (3 bytes each, hex)
    MacroRaw { slot: u8, events: String },
    /// Send any register frame (experts only)
    Raw { register: String, payload: Option<String> },
    /// Save, load and inspect named copies of the settings
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// Print a shell completion script
    Completions { shell: clap_complete::Shell },
    #[command(hide = true)]
    Man,
}

#[derive(Args, Debug)]
pub struct Brightness {
    /// 0 (dimmest) to 3 (brightest)
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub brightness: u8,
}

#[derive(Subcommand, Debug)]
pub enum LightCmd {
    /// Main lights off
    Off,
    /// One steady colour
    Steady {
        color: String,
        #[command(flatten)]
        b: Brightness,
    },
    /// Cycle through 1 to 7 colours (rrggbb,rrggbb,...)
    Cycle {
        colors: String,
        #[command(flatten)]
        b: Brightness,
        /// 0 (fastest) to 3 (slowest)
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=3))]
        speed: u8,
    },
    /// One colour that fades in and out slowly
    Breathing {
        color: String,
        #[command(flatten)]
        b: Brightness,
    },
    /// One colour that flashes on and off quickly
    Flash {
        color: String,
        #[command(flatten)]
        b: Brightness,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProfileCmd {
    /// Store the settings m7ctl last wrote (no name: overwrite the active profile)
    Save {
        name: Option<String>,
    },
    /// Write a stored profile to the mouse; it becomes the active profile
    Load {
        name: String,
    },
    /// The active profile and whether it differs from the current settings
    Current,
    /// List profiles (* = active)
    List,
    /// Print a profile (no name: the current settings)
    Show {
        name: Option<String>,
    },
    /// Convert a vendor .psf profile into a profile (nothing is sent; use `profile load`)
    Import {
        file: std::path::PathBuf,
        /// Profile name (default: the name inside the file)
        name: Option<String>,
    },
    Rename {
        old: String,
        new: String,
    },
    Delete {
        name: String,
    },
}

/// Opens the mouse at a path, or detects it when `None`.
pub type Opener = Arc<dyn Fn(Option<&str>) -> Result<Mouse, String> + Send + Sync>;

/// Everything a command needs from the outside world. Tests swap in a fake mouse and a temporary store.
#[derive(Clone)]
pub struct Context {
    pub store: Store,
    /// Explicit HID device path (`--device`), if any.
    pub device: Option<String>,
    /// Opens the mouse; `None` means "detect it".
    pub opener: Opener,
    /// Progress messages such as "the mouse is asleep".
    pub notice: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Context {
    /// The real thing: HID devices, the user's configuration directory, messages on stderr.
    pub fn system(device: Option<String>) -> Result<Context, String> {
        Ok(Context { store: Store::default_location()?, device, opener: Arc::new(Mouse::open), notice: Arc::new(|m| eprintln!("{m}")) })
    }

    pub fn mouse(&self) -> Result<Mouse, String> {
        (self.opener)(self.device.as_deref()).map(|m| m.with_notice(self.notice.clone()))
    }

    /// Remember a setting; a failure is reported but never fails the command.
    pub(crate) fn record(&self, key: &str, args: Vec<String>) {
        if let Err(e) = self.store.record(key, args) {
            (self.notice)(&format!("note: {e}"));
        }
    }

    pub(crate) fn forget(&self, key: &str) {
        if let Err(e) = self.store.forget(key) {
            (self.notice)(&format!("note: {e}"));
        }
    }
}

/// The man page as roff text.
pub fn man_page() -> Result<String, String> {
    let mut buf = Vec::new();
    clap_mangen::Man::new(Cli::command()).render(&mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Parse and run one command line (without the program name). Normal output goes to `out`.
pub fn execute(args: &[String], ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    let argv = std::iter::once("m7ctl".to_string()).chain(args.iter().cloned());
    let mut cli = Cli::try_parse_from(argv).map_err(|e| e.render().to_string().trim_end().to_string())?;
    let mut ctx = ctx.clone();
    if cli.device.is_some() {
        ctx.device = cli.device.take();
    }
    run(cli.command, &ctx, out)
}

pub fn run(command: Command, ctx: &Context, out: &mut dyn Write) -> Result<(), String> {
    match command {
        Command::List { verbose } => info::list(verbose, out),
        Command::Status { json } => info::status(ctx, json, out),
        Command::Profile(p) => profile::run(p, ctx, out),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "m7ctl", &mut *out);
            Ok(())
        }
        Command::Man => write!(out, "{}", man_page()?).map_err(|e| e.to_string()),
        other => apply::run(other, ctx, out),
    }
}
