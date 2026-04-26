// SPDX-License-Identifier: GPL-3.0

//! Pure-Rust MPRIS client using zbus D-Bus proxies.
//!
//! Discovers active media players on the session bus and retrieves
//! their metadata (track title, artist) via the standard
//! `org.mpris.MediaPlayer2.Player` interface.

use std::collections::HashMap;
use zbus::Connection;

/// Metadata retrieved from an MPRIS player.
#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
}

/// Find the first active MPRIS media player and return its track metadata.
///
/// Returns `None` if no player is found or no metadata is available.
pub async fn get_active_track() -> Option<TrackMetadata> {
    let connection = Connection::session().await.ok()?;
    let dbus_proxy = zbus::fdo::DBusProxy::new(&connection).await.ok()?;

    // List all bus names and filter for MPRIS players.
    let names = dbus_proxy.list_names().await.ok()?;
    let mpris_name = names
        .iter()
        .find(|name| name.as_str().starts_with("org.mpris.MediaPlayer2."))?;

    // Build a generic proxy for the Player interface to read Metadata.
    let player_proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(&connection)
        .destination(mpris_name.as_str())
        .ok()?
        .path("/org/mpris/MediaPlayer2")
        .ok()?
        .interface("org.mpris.MediaPlayer2.Player")
        .ok()?
        .build()
        .await
        .ok()?;

    let metadata: HashMap<String, zbus::zvariant::OwnedValue> = player_proxy
        .get_property("Metadata")
        .await
        .ok()?;

    let title = metadata
        .get("xesam:title")
        .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
        .unwrap_or_default();

    let artist = metadata
        .get("xesam:artist")
        .and_then(|v| {
            // xesam:artist is an array of strings — take the first one.
            <Vec<String> as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone())
                .ok()
                .and_then(|artists| artists.into_iter().next())
        })
        .unwrap_or_default();

    Some(TrackMetadata { title, artist })
}
