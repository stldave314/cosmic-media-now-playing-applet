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

/// Thickness in pixels of the optional progress line along the bottom of the
/// panel strip.
pub const PANEL_PROGRESS_HEIGHT: f32 = 2.0;

/// Gap in pixels between the progress line and the bottom edge of the panel,
/// so the hover highlight's rounded corners don't clip it.
pub const PANEL_PROGRESS_INSET: u16 = 3;

/// How often the panel progress line is redrawn while playing. One pixel of a
/// 200-pixel line is about a second of a three-minute track, so faster would
/// only cost wake-ups.
pub const PANEL_PROGRESS_TICK_MS: u64 = 1_000;

/// How much one notch of the mouse wheel changes the player's volume, as a
/// fraction of full scale.
pub const VOLUME_STEP: f64 = 0.05;

/// Pixels of smooth (touchpad) scrolling that count as one wheel notch.
pub const SCROLL_PIXELS_PER_STEP: f32 = 24.0;

/// Shortest gap between two track skips made with the wheel. A flick of a
/// free-spinning wheel or a touchpad swipe sends many notches at once; without
/// this it would skip several tracks.
pub const SCROLL_SKIP_COOLDOWN_MS: u64 = 400;

/// How long a short notice (such as the new volume) replaces the title on the
/// panel.
pub const PANEL_NOTICE_MS: u64 = 1_200;

// ── Media popup ─────────────────────────────────────────────────────────────

/// Width of the media popup. libcosmic fixes every applet popup at this width
/// (see `popup_container`); the backdrop's proportions are derived from it.
pub const POPUP_WIDTH: f32 = 360.0;

/// Tallest the settings view grows before it scrolls, in logical pixels.
/// The popup's own height limit doesn't constrain an autosized surface, so
/// the scroll area carries it; this fits a 768-pixel-high screen with the
/// panel.
pub const SETTINGS_MAX_HEIGHT: f32 = 560.0;

/// Tracks at least this long (in microseconds) get the podcast controls —
/// skip back, skip forward, and playback speed — in the popup. Ten minutes
/// separates episodes, audiobook chapters, and lectures from songs.
pub const LONG_TRACK_US: i64 = 10 * 60 * 1_000_000;

/// How far the podcast controls jump back, in seconds. Shorter than the jump
/// forward, as in most podcast apps: going back is for catching a missed
/// sentence, going forward for skipping a segment.
pub const SKIP_BACK_SECS: i64 = 15;

/// How far the podcast controls jump forward, in seconds.
pub const SKIP_FORWARD_SECS: i64 = 30;

/// Playback speeds the speed button cycles through, slowest first. Only the
/// ones inside the player's own MinimumRate..=MaximumRate are offered.
pub const PLAYBACK_RATES: [f64; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];

/// Edge length in pixels of the album-art card in the media popup.
pub const POPUP_ART_SIZE: f32 = 220.0;

/// Edge length in pixels of the square the cover is cropped to for the art
/// card. Twice [`POPUP_ART_SIZE`], so the card stays sharp at up to 200%
/// display scaling. The cover is cropped up front because the image widget
/// doesn't clip a cover-fitted image to its bounds: a wide video thumbnail
/// would otherwise spill out across the popup.
///
/// Smaller covers are scaled *up* to this with a smooth filter. Browsers often
/// publish tiny art (Firefox-based ones hand over 60 × 60 for YouTube Music),
/// and the renderer's own upscaling of that looks far blockier.
pub const ART_CARD_PIXELS: u32 = 440;

/// Size in pixels of the blurred backdrop image. The cover is shrunk to this
/// before blurring and the renderer stretches it back up to fill the popup:
/// blur throws away exactly the detail that shrinking loses, so the result
/// looks the same as blurring the full-size cover at a fraction of the cost.
///
/// Portrait, roughly the popup's own shape, because the backdrop is stretched
/// to the popup's exact bounds rather than cover-fitted: the software renderer
/// draws a cover-fitted image past its bounds, which would put the image's
/// corners outside the popup.
///
/// Fine enough that one backdrop pixel (2.5 logical pixels across) is smaller
/// than [`BACKDROP_CORNER_MARGIN`], so the cut corners land cleanly inside the
/// popup's curve.
pub const BACKDROP_WIDTH: u32 = 144;
pub const BACKDROP_HEIGHT: u32 = 224;

/// Gaussian blur strength applied to the backdrop, in backdrop pixels. The
/// popup is two and a half times wider than [`BACKDROP_WIDTH`], so on screen
/// this is about two and a half times larger.
pub const BACKDROP_BLUR_SIGMA: f32 = 11.2;

/// How much rounder than the popup's own corners the backdrop's corners are
/// cut, in logical pixels. The software renderer can't round a scaled image's
/// corners to match the popup, so the backdrop carries its own transparent
/// corners, cut slightly wider so that no image ever shows outside the
/// popup's curve. The sliver this leaves is filled with the cover's average
/// color, so it doesn't show.
///
/// The margin also has to absorb the renderer's smoothing when it stretches
/// the backdrop, which carries the cut edge about two backdrop pixels
/// (five logical pixels) outward.
pub const BACKDROP_CORNER_MARGIN: f32 = 6.0;

/// The dark gradient drawn between the backdrop and the popup's content. It is
/// what keeps white text and controls readable on any cover, including a
/// pure-white one, so the opacities are the trade-off between a vivid backdrop
/// and guaranteed contrast. Each entry is `(position, black opacity)`, with
/// position running from the top of the popup (0.0) to the bottom (1.0).
///
/// The art card and header pills sit in the lighter top part; all text sits
/// below the 0.62 stop, where white text on a white cover is still about 8:1.
pub const BACKDROP_SCRIM: [(f32, f32); 3] = [(0.0, 0.45), (0.62, 0.72), (1.0, 0.80)];

/// Largest album-art width or height accepted, checked from the image header
/// before anything is decoded. A few kilobytes of compressed data can claim
/// enormous dimensions, and decoding allocates width × height × 4 bytes, so
/// art beyond this is refused rather than decoded. Real cover art is far
/// smaller; 4096² still costs 64 MiB once decoded.
pub const MAX_ART_DIMENSION: u32 = 4096;

// ── Timing ──────────────────────────────────────────────────────────────────

/// How often the applet re-reads every player while the media popup is open,
/// so the seek bar and times stay in step with what the player reports.
pub const MPRIS_POLL_INTERVAL_MS: u64 = 1_000;

/// Fallback re-read interval while the popup is closed. Normally the applet
/// refreshes only when a player announces a change (MPRIS requires
/// `PropertiesChanged` and `Seeked`), so this exists only to catch players that
/// forget to; it is deliberately slow so an idle desktop isn't woken up for
/// nothing.
pub const MPRIS_IDLE_POLL_INTERVAL_MS: u64 = 10_000;

/// After a change signal arrives, wait this long before re-reading, so a burst
/// of signals (a track change typically sends several) costs one refresh.
pub const MPRIS_SIGNAL_DEBOUNCE_MS: u64 = 40;

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

/// Sleep-timer lengths offered in the popup, in minutes.
pub const SLEEP_TIMER_OPTIONS: [u32; 6] = [15, 30, 45, 60, 90, 120];

/// How often a running sleep timer wakes the popup to update its countdown,
/// in seconds. The countdown is shown in whole minutes.
pub const SLEEP_TIMER_REDRAW_SECS: u64 = 30;

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
