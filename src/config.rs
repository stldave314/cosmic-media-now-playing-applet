// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// How the track information is formatted for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DisplayFormat {
    TitleOnly,
    #[default]
    ArtistTitle,
    TitleArtist,
    /// The user's own template; see [`render_template`].
    Custom,
}

/// The template a new custom format starts from.
pub const DEFAULT_CUSTOM_FORMAT: &str = "{artist} — {title} ({album})";

impl DisplayFormat {
    /// Formats the track info according to this display format. `template`
    /// is only used by [`DisplayFormat::Custom`].
    pub fn format(self, title: &str, artist: &str, album: &str, template: &str) -> String {
        if self == Self::Custom {
            return render_template(template, title, artist, album);
        }
        if artist.is_empty() {
            return title.to_string();
        }
        match self {
            Self::TitleOnly => title.to_string(),
            Self::ArtistTitle => format!("{artist} — {title}"),
            Self::TitleArtist | Self::Custom => format!("{title} — {artist}"),
        }
    }
}

/// Fill a custom format: `{title}`, `{artist}`, and `{album}` are replaced
/// by the track's values, and everything else is kept as typed.
///
/// The template is read in a single pass, so a title that happens to contain
/// `{artist}` stays literal rather than being substituted again. Fields a
/// player doesn't publish (often the album) leave gaps, which are tidied:
/// brackets left empty are dropped, and separators left dangling at either
/// end or doubled up in the middle are removed, so `{artist} — {title}
/// ({album})` reads `Artist — Title` without an album.
pub fn render_template(template: &str, title: &str, artist: &str, album: &str) -> String {
    let fields = [("{title}", title), ("{artist}", artist), ("{album}", album)];
    let mut filled = String::with_capacity(template.len() + title.len() + artist.len() + album.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        let tail = &rest[start..];
        match fields.iter().find(|(token, _)| tail.starts_with(token)) {
            Some((token, value)) => {
                filled.push_str(value);
                rest = &tail[token.len()..];
            }
            None => {
                filled.push('{');
                rest = &tail[1..];
            }
        }
    }
    filled.push_str(rest);
    tidy_separators(&drop_empty_brackets(&filled))
}

/// Characters that only separate fields; a word made solely of these is a
/// separator.
const SEPARATORS: &[char] = &['—', '–', '-', '·', '•', '|', '/', ',', ':', ';'];

/// Remove `()` and `[]` pairs holding nothing but whitespace, repeatedly, so
/// nested empties go too.
fn drop_empty_brackets(text: &str) -> String {
    let mut out = text.to_string();
    loop {
        let mut changed = false;
        for (open, close) in [('(', ')'), ('[', ']')] {
            let mut search_from = 0;
            while let Some(offset) = out[search_from..].find(open) {
                let at = search_from + offset;
                let inner_start = at + open.len_utf8();
                let inner_len = out[inner_start..].len() - out[inner_start..].trim_start().len();
                if out[inner_start + inner_len..].starts_with(close) {
                    out.replace_range(at..inner_start + inner_len + close.len_utf8(), "");
                    changed = true;
                } else {
                    search_from = inner_start;
                }
            }
        }
        if !changed {
            return out;
        }
    }
}

/// Collapse runs of whitespace, then drop separator words at either end and
/// any that directly follow another, and finally separator characters left
/// clinging to the last word.
fn tidy_separators(text: &str) -> String {
    let is_separator = |word: &str| word.chars().all(|c| SEPARATORS.contains(&c));
    let mut words: Vec<&str> = Vec::new();
    for word in text.split_whitespace() {
        if is_separator(word) && words.last().is_none_or(|last| is_separator(last)) {
            continue;
        }
        words.push(word);
    }
    while words.last().is_some_and(|last| is_separator(last)) {
        words.pop();
    }
    words.join(" ").trim_end_matches(SEPARATORS).to_string()
}

/// Which leading element to show beside the scrolling text in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PanelIcon {
    /// Album artwork thumbnail, falling back to the music-note icon when none.
    #[default]
    AlbumArt,
    /// Always the generic music-note icon.
    MusicNote,
    /// No leading element at all (takes up no space).
    None,
}

/// What the mouse wheel does over the panel applet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScrollAction {
    /// Raise (wheel up) or lower (wheel down) the player's own volume.
    #[default]
    Volume,
    /// Go to the next track (wheel down) or the previous one (wheel up).
    SkipTrack,
    /// Leave the wheel alone.
    Nothing,
}

/// A player the user has hidden, remembered across sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenPlayer {
    /// Stable identity derived from the bus name (see
    /// [`crate::mpris::player_key`]), so a new browser instance stays hidden.
    pub key: String,
    /// The player's display name when it was hidden, for the settings list
    /// while it isn't running.
    pub name: String,
}

/// Persistent configuration for the Now Playing applet.
///
/// Stored and loaded automatically via `cosmic-config`.
#[derive(Debug, Clone, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct NowPlayingConfig {
    /// Width of the applet widget on the panel, in pixels (100..=500).
    pub widget_width: u32,
    /// Scroll speed level 1 (slowest) – 10 (fastest). Stored as tick interval ms = 330 - level*30.
    pub scroll_speed: u32,
    /// How track metadata is formatted for display.
    pub display_format: DisplayFormat,
    /// Top margin in pixels to shift the text vertically within the applet (-10..=20).
    pub top_margin: i32,
    /// Left margin in pixels, inset before the panel content (0..=40).
    pub left_margin: i32,
    /// Right margin in pixels, inset after the panel content (0..=40).
    pub right_margin: i32,
    /// The specific MPRIS bus name the user has chosen to control, if any.
    pub selected_player: Option<String>,
    /// Which leading element to show beside the panel text.
    pub panel_icon: PanelIcon,
    /// Size in pixels of the album-art thumbnail in the panel (12..=48).
    pub panel_art_size: u32,
    /// Show playback control buttons in the panel while hovering. Only takes
    /// effect when `panel_icon` is not `PanelIcon::None`.
    pub show_hover_controls: bool,
    /// Gap in pixels between the leading icon/art and the scrolling title
    /// (0..=40). Only meaningful when `panel_icon` is not `PanelIcon::None`.
    pub icon_spacing: u32,
    /// Stop the applet automatically once playback has been idle this many
    /// minutes (0..=60), clearing the title and album art so a finished session
    /// stops occupying the panel — the same result as pressing STOP. `0`
    /// disables it, which is the default: the applet keeps showing the last
    /// track until the user stops it explicitly.
    pub idle_clear_minutes: u32,
    /// What scrolling over the panel applet does.
    pub scroll_action: ScrollAction,
    /// Middle-clicking the panel applet toggles play/pause.
    pub middle_click_play_pause: bool,
    /// Follow whichever player starts playing. When off, the applet stays on
    /// the player chosen in the popup until that player goes away.
    pub auto_switch: bool,
    /// Players never shown on the panel, followed, or listed in the popup.
    pub hidden_players: Vec<HiddenPlayer>,
    /// The template used by [`DisplayFormat::Custom`].
    pub custom_format: String,
}

impl NowPlayingConfig {
    /// Whether the player with this key is hidden.
    pub fn is_hidden(&self, key: &str) -> bool {
        self.hidden_players.iter().any(|p| p.key == key)
    }

    /// Clamp every numeric field to its documented range.
    ///
    /// The sliders can only produce in-range values, but config also arrives
    /// from the on-disk file via the cosmic-config watcher — which anyone (or
    /// any tool) can hand-edit — so the ranges are enforced at the boundary
    /// rather than trusted.
    pub fn clamped(mut self) -> Self {
        self.widget_width = self.widget_width.clamp(100, 500);
        self.scroll_speed = self.scroll_speed.min(10);
        self.top_margin = self.top_margin.clamp(-10, 20);
        self.left_margin = self.left_margin.clamp(0, 40);
        self.right_margin = self.right_margin.clamp(0, 40);
        self.panel_art_size = self.panel_art_size.clamp(12, 48);
        self.icon_spacing = self.icon_spacing.min(40);
        self.idle_clear_minutes = self.idle_clear_minutes.min(60);
        self
    }
}

impl Default for NowPlayingConfig {
    fn default() -> Self {
        Self {
            widget_width: 200,
            scroll_speed: 5,
            display_format: DisplayFormat::default(),
            top_margin: 0,
            // A little inset by default so the icon isn't flush against the
            // neighboring applet.
            left_margin: 5,
            right_margin: 0,
            selected_player: None,
            panel_icon: PanelIcon::default(),
            panel_art_size: 16,
            show_hover_controls: true,
            icon_spacing: 6,
            // Off by default: stopping hides the applet, and that shouldn't
            // happen to anyone who didn't ask for it.
            idle_clear_minutes: 0,
            scroll_action: ScrollAction::default(),
            middle_click_play_pause: true,
            auto_switch: true,
            hidden_players: Vec::new(),
            custom_format: DEFAULT_CUSTOM_FORMAT.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_fills_every_field() {
        assert_eq!(
            render_template("{artist} — {title} ({album})", "Song", "Band", "Record"),
            "Band — Song (Record)"
        );
        assert_eq!(render_template("{title}", "Song", "Band", "Record"), "Song");
        assert_eq!(render_template("[{album}] {title}", "Song", "Band", "Record"), "[Record] Song");
    }

    #[test]
    fn missing_fields_leave_no_empty_brackets_or_dangling_separators() {
        let t = "{artist} — {title} ({album})";
        assert_eq!(render_template(t, "Song", "Band", ""), "Band — Song");
        assert_eq!(render_template(t, "Song", "", "Record"), "Song (Record)");
        assert_eq!(render_template("{artist} · {album} · {title}", "Song", "Band", ""), "Band · Song");
        assert_eq!(render_template("{title}, {artist}", "Song", "", ""), "Song");
        assert_eq!(render_template("[{album}] {title}", "Song", "", ""), "Song");
    }

    #[test]
    fn metadata_cannot_inject_placeholders() {
        // A title that contains a placeholder stays literal.
        assert_eq!(render_template("{title} / {artist}", "{artist}", "Band", ""), "{artist} / Band");
        // Unknown placeholders and stray braces are kept as typed.
        assert_eq!(render_template("{year} {title}", "Song", "", ""), "{year} Song");
        assert_eq!(render_template("{ {title}", "Song", "", ""), "{ Song");
    }

    #[test]
    fn built_in_formats_are_unchanged() {
        assert_eq!(DisplayFormat::ArtistTitle.format("Song", "Band", "Record", ""), "Band — Song");
        assert_eq!(DisplayFormat::TitleArtist.format("Song", "Band", "", ""), "Song — Band");
        assert_eq!(DisplayFormat::TitleOnly.format("Song", "Band", "", ""), "Song");
        assert_eq!(DisplayFormat::ArtistTitle.format("Song", "", "", ""), "Song");
    }
}
