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
}

impl DisplayFormat {
    /// Formats the track info according to this display format.
    pub fn format(self, title: &str, artist: &str) -> String {
        if artist.is_empty() {
            return title.to_string();
        }
        match self {
            Self::TitleOnly => title.to_string(),
            Self::ArtistTitle => format!("{artist} — {title}"),
            Self::TitleArtist => format!("{title} — {artist}"),
        }
    }
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
}

impl NowPlayingConfig {
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
            // neighbouring applet.
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
        }
    }
}
