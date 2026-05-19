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
    /// The specific MPRIS bus name the user has chosen to control, if any.
    pub selected_player: Option<String>,
}

impl Default for NowPlayingConfig {
    fn default() -> Self {
        Self {
            widget_width: 200,
            scroll_speed: 5,
            display_format: DisplayFormat::default(),
            top_margin: 0,
            selected_player: None,
        }
    }
}
