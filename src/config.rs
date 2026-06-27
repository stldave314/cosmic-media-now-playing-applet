// SPDX-License-Identifier: GPL-3.0

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// How the track information is formatted for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayFormat {
    TitleOnly,
    ArtistTitle,
    TitleArtist,
}

impl Default for DisplayFormat {
    fn default() -> Self {
        Self::ArtistTitle
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelIcon {
    /// Album artwork thumbnail, falling back to the music-note icon when none.
    AlbumArt,
    /// Always the generic music-note icon.
    MusicNote,
    /// No leading element at all (takes up no space).
    None,
}

impl Default for PanelIcon {
    fn default() -> Self {
        Self::AlbumArt
    }
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
}

impl Default for NowPlayingConfig {
    fn default() -> Self {
        Self {
            widget_width: 200,
            scroll_speed: 5,
            display_format: DisplayFormat::default(),
            top_margin: 0,
            left_margin: 0,
            right_margin: 0,
            selected_player: None,
            panel_icon: PanelIcon::default(),
            panel_art_size: 16,
            show_hover_controls: true,
        }
    }
}
