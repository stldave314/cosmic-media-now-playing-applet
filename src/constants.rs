// SPDX-License-Identifier: GPL-3.0-only

//! Compile-time tuning values for the applet, gathered in one place.
//!
//! These are *implementation* tuning knobs, not user settings — anything the
//! user should be able to change lives in [`crate::config`] and is persisted via
//! `cosmic-config`. Keeping these compile-time avoids a second configuration
//! mechanism, a startup file read, and a class of "malformed config" failures,
//! while still giving one obvious place to find and adjust them.

// ── Panel layout ────────────────────────────────────────────────────────────

/// The separator inserted between repetitions of scrolling text.
pub const SCROLL_GAP: &str = "    ·    ";

/// Approximate character width in pixels, used only to decide *whether* the
/// title overflows and needs scrolling. Deliberately on the generous side:
/// over-estimating makes the applet scroll a title that might just have fitted,
/// which is far less bad than not scrolling one that doesn't fit and silently
/// hiding its end.
pub const APPROX_CHAR_WIDTH: f32 = 8.0;

/// Extra characters handed to the panel text beyond the estimated fit.
///
/// The text container clips at its real pixel boundary, so supplying surplus
/// characters lets it fill the full width exactly and the estimate above only
/// has to be in the right ballpark. Without this the title stops wherever the
/// estimate under-counts — which it always does for a proportional font —
/// leaving visible dead space before the edge of the applet.
pub const TEXT_OVERDRAW_CHARS: usize = 16;

/// Size in pixels of the music-note fallback icon in the panel.
pub const MUSIC_NOTE_SIZE: f32 = 16.0;

/// Approximate width in pixels of one panel playback-control button, including
/// its spacing. Used to decide whether all three controls fit.
pub const CONTROL_BUTTON_WIDTH: f32 = 34.0;

// ── Timing ──────────────────────────────────────────────────────────────────

/// How often the applet polls MPRIS for player state.
pub const MPRIS_POLL_INTERVAL_MS: u64 = 1_000;

/// Tick interval for smoothly advancing the popup progress bar between polls
/// (~10 fps). Only runs while the popup is open and a track is playing.
pub const PROGRESS_TICK_MS: u64 = 100;

/// How long the width slider must be still before the change is committed and
/// the panel is resized, so dragging doesn't spam resize requests.
pub const WIDTH_SETTLE_MS: u64 = 1_500;

/// Scroll timer bounds: tick interval = `SCROLL_TICK_BASE_MS - level * SCROLL_TICK_STEP_MS`,
/// clamped to at least `SCROLL_TICK_MIN_MS`, for speed levels 1–10.
pub const SCROLL_TICK_BASE_MS: u64 = 330;
pub const SCROLL_TICK_STEP_MS: u64 = 30;
pub const SCROLL_TICK_MIN_MS: u64 = 30;

// ── "Pause before playing next track" ──────────────────────────────────────────

/// How far before the end of the current track the armed watcher pauses.
///
/// Stopping slightly *early* avoids racing the track change altogether: the
/// player never advances, so none of the next track can be heard. The trade-off
/// is losing this much of the current track's tail, which is imperceptible.
pub const PAUSE_LEAD_US: i64 = 300_000;

/// Poll interval used when the track duration is unknown (live streams), where
/// the only available signal is the track actually changing.
pub const ARMED_WATCH_POLL_MS: u64 = 50;

/// Longest the armed watcher sleeps while waiting for the end of a track. It
/// sleeps just long enough to reach the pause point, capped by this so it still
/// notices seeks and manual pauses.
pub const ARMED_WATCH_MAX_SLEEP_MS: u64 = 500;


// ── Untrusted-input limits ──────────────────────────────────────────────────
// Album art URLs (file://, data:, http(s)://) come from MPRIS metadata, which
// any process on the session bus can publish — so every art source is treated
// as hostile and bounded. Without caps, a peer could hand the applet
// /dev/zero, a FIFO, or an endless HTTP body and wedge or OOM it.

/// Largest album-art file read from disk. Real cover art tops out around a few
/// megabytes; anything bigger is refused rather than buffered.
pub const MAX_ART_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// Largest album-art HTTP download, enforced while streaming — a response that
/// keeps going past this is abandoned, not buffered.
pub const MAX_ART_HTTP_BYTES: u64 = 20 * 1024 * 1024;

// ── Network ─────────────────────────────────────────────────────────────────

/// Total per-request timeout for album-art downloads and the update check, so
/// a stalled server can't pin a background task open indefinitely.
pub const HTTP_TIMEOUT_SECS: u64 = 15;

/// User agent sent with album-art and update requests. Derived from the package
/// metadata so it can't drift out of date.
pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Consulted by the update check, derived from the `repository` field in
/// Cargo.toml so the URL has a single source of truth.
///
/// This deliberately points at the latest *release* rather than the version in
/// `main`: the two diverge as soon as a version-bumping commit lands, and
/// telling users about a version they cannot download is worse than saying
/// nothing. GitHub redirects this to `/releases/tag/<tag>`, so the published
/// tag can be read straight off the final URL — no JSON parsing needed. A repo
/// with no releases yet answers 404 and doesn't redirect.
pub const UPDATE_LATEST_RELEASE_URL: &str =
    concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/latest");

/// Template for YouTube thumbnails, used when a browser's art file is not
/// reachable. `{id}` is replaced with the video id.
pub const YOUTUBE_THUMBNAIL_URL: &str = "https://img.youtube.com/vi/{id}/hqdefault.jpg";

/// Sandboxed players whose private `/tmp` is searched as a last resort when
/// reading album art.
pub const SNAP_ART_PACKAGES: &[&str] = &[
    "chromium", "chromium-browser", "firefox", "spotify",
    "epiphany", "brave", "vivaldi", "opera",
];
