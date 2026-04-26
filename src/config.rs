// SPDX-License-Identifier: GPL-3.0

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// How fast the text scrolls when it overflows the widget width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScrollSpeed {
    Slow,
    Medium,
    Fast,
}

impl Default for ScrollSpeed {
    fn default() -> Self {
        Self::Medium
    }
}

impl ScrollSpeed {
    /// Returns the scroll tick interval in milliseconds.
    pub fn tick_ms(self) -> u64 {
        match self {
            Self::Slow => 80,
            Self::Medium => 50,
            Self::Fast => 30,
        }
    }
}

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
    /// How fast the text scrolls when it overflows.
    pub scroll_speed: ScrollSpeed,
    /// How track metadata is formatted for display.
    pub display_format: DisplayFormat,
    /// Top margin in pixels to shift the text vertically within the applet (-10..=20).
    pub top_margin: i32,
}

impl Default for NowPlayingConfig {
    fn default() -> Self {
        Self {
            widget_width: 200,
            scroll_speed: ScrollSpeed::default(),
            display_format: DisplayFormat::default(),
            top_margin: 0,
        }
    }
}
