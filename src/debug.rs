// SPDX-License-Identifier: GPL-3.0-only

//! Build-time diagnostic logging.
//!
//! The applet's stderr is piped to `cosmic-panel` and ends up on the session
//! TTY, and this system has no persistent journal — so `eprintln!` diagnostics
//! are effectively unreadable in normal use. Everything here goes straight to a
//! file instead.
//!
//! Logging is gated on the [`ENABLED`] constant so it can be compiled out
//! entirely: when it is `false` the `debug_log!` macro's body is unreachable and
//! the optimiser removes it, leaving no formatting cost and no file I/O. The
//! arguments are still type-checked either way, so disabled call sites can't rot.
//!
//! ```ignore
//! debug_log!(MPRIS, "found {} players", players.len());
//! ```

/// Developer switch — flip this to turn diagnostic logging on or off locally.
///
/// This is *not* the final word: see [`ENABLED`], which additionally forces
/// logging off for release builds.
const DEVELOPER_LOGGING: bool = false;

/// Whether logging actually happens.
///
/// Release packages are built with the `release-build` feature (see the
/// packaging targets in `install.sh`), which forces this to `false` no matter
/// what [`DEVELOPER_LOGGING`] says — so a release can never ship with diagnostic
/// logging left switched on by accident.
pub const ENABLED: bool = DEVELOPER_LOGGING && !cfg!(feature = "release-build");

/// File name of the log. See [`path`] for where it lands.
pub const FILE_NAME: &str = "cosmic-media-now-playing.log";

/// Where the log is written. Truncated once per process launch.
///
/// Preferably `$XDG_RUNTIME_DIR` (per-user, mode 0700), so other local users
/// can neither read the log — it records listening history — nor pre-create
/// the path to redirect our writes. Only when that variable is unset (odd for
/// a desktop session) does it fall back to `/tmp`; the 0600 + `O_NOFOLLOW`
/// open in [`write`] keeps that fallback safe against symlink planting too.
pub fn path() -> &'static std::path::Path {
    use std::sync::OnceLock;
    static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        dir.join(FILE_NAME)
    })
}

// ── Categories ──────────────────────────────────────────────────────────────
// Short tags so a run can be filtered with `grep`.

/// MPRIS discovery, metadata, and commands.
pub const MPRIS: &str = "mpris";
/// The "pause before playing next track" watcher.
pub const WATCH: &str = "watch";
/// Album-art resolution and fetching.
pub const ART: &str = "art";
/// Popup, panel, and settings interactions.
pub const UI: &str = "ui";
/// Configuration load/save.
pub const CONFIG: &str = "config";
/// Version update checks.
pub const UPDATE: &str = "update";

/// Append one line, prefixed with the category and seconds since process start.
///
/// Prefer the [`debug_log!`](crate::debug_log) macro, which skips formatting
/// entirely when [`ENABLED`] is `false`.
pub fn write(category: &str, msg: &str) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::OnceLock;

    // Belt and braces: the macro already skips disabled calls, but gating here
    // too makes every string in this function (including the log file name)
    // provably dead code in release builds, so none of it reaches the binary.
    if !ENABLED {
        return;
    }

    // First call truncates the file so each launch starts clean, and anchors the
    // elapsed-time clock.
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    let mut first = false;
    let start = START.get_or_init(|| {
        first = true;
        std::time::Instant::now()
    });

    // 0600 — the log records listening history, which is nobody else's
    // business; O_NOFOLLOW — refuse a symlink planted at the path by another
    // local user (relevant for the /tmp fallback), rather than following it
    // and overwriting whatever it points at.
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(!first)
        .write(true)
        .truncate(first)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path())
    {
        Ok(f) => f,
        Err(_) => return,
    };

    if first {
        let _ = writeln!(
            file,
            "=== {} v{} — debug log ===",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        );
    }

    let _ = writeln!(file, "[{:9.3}] {category:<6} {msg}", start.elapsed().as_secs_f64());
}

/// Write a formatted line to the debug log, compiled out when
/// [`ENABLED`] is `false`.
#[macro_export]
macro_rules! debug_log {
    ($category:expr, $($arg:tt)*) => {{
        if $crate::debug::ENABLED {
            $crate::debug::write($category, &format!($($arg)*));
        }
    }};
}
