// SPDX-License-Identifier: GPL-3.0

//! Pure-Rust MPRIS client using zbus D-Bus proxies.
//!
//! Discovers active media players on the session bus and retrieves
//! their metadata (track title, artist) via the standard
//! `org.mpris.MediaPlayer2.Player` interface.

use std::collections::HashMap;
use zbus::Connection;

/// Metadata retrieved from an MPRIS player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub art_url: Option<String>,
}

/// Commands that can be sent to the MPRIS player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MprisCommand {
    PlayPause,
    Next,
    Previous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    pub bus_name: String,
    pub identity: String,
    pub metadata: TrackMetadata,
    pub playback_status: String,
}

/// Find all active MPRIS media players and return their information.
pub async fn get_all_players() -> Vec<PlayerInfo> {
    let mut players = Vec::new();
    let Ok(connection) = Connection::session().await else {
        return players;
    };
    let Ok(dbus_proxy) = zbus::fdo::DBusProxy::new(&connection).await else {
        return players;
    };

    let Ok(names) = dbus_proxy.list_names().await else {
        return players;
    };

    for name in names {
        if !name.as_str().starts_with("org.mpris.MediaPlayer2.") {
            continue;
        }
        let bus_name = name.to_string();

        let Ok(root_proxy) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(&connection)
            .destination(bus_name.as_str())
            .unwrap()
            .path("/org/mpris/MediaPlayer2")
            .unwrap()
            .interface("org.mpris.MediaPlayer2")
            .unwrap()
            .build()
            .await else { continue; };

        let identity: String = root_proxy
            .get_property::<zbus::zvariant::OwnedValue>("Identity")
            .await
            .ok()
            .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or_else(|| "Unknown Player".to_string());

        let Ok(player_proxy) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(&connection)
            .destination(bus_name.as_str())
            .unwrap()
            .path("/org/mpris/MediaPlayer2")
            .unwrap()
            .interface("org.mpris.MediaPlayer2.Player")
            .unwrap()
            .build()
            .await else { continue; };

        let metadata = if let Ok(metadata_map) = player_proxy.get_property::<HashMap<String, zbus::zvariant::OwnedValue>>("Metadata").await {
            let title = metadata_map
                .get("xesam:title")
                .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
                .unwrap_or_default();

            let artist = metadata_map
                .get("xesam:artist")
                .and_then(|v| {
                    <Vec<String> as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone())
                        .ok()
                        .and_then(|artists| artists.into_iter().next())
                })
                .unwrap_or_default();

            let art_url = metadata_map
                .get("mpris:artUrl")
                .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok());

            TrackMetadata { title, artist, art_url }
        } else {
            TrackMetadata::default()
        };

        let playback_status: String = player_proxy
            .get_property::<zbus::zvariant::OwnedValue>("PlaybackStatus")
            .await
            .ok()
            .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or_else(|| "Stopped".to_string());

        players.push(PlayerInfo {
            bus_name,
            identity,
            metadata,
            playback_status,
        });
    }

    players
}

/// Sends a command to a specific MPRIS media player.
pub async fn send_command(bus_name: String, command: MprisCommand) {
    if let Ok(connection) = Connection::session().await {
        if let Ok(player_proxy) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(&connection)
            .destination(bus_name.as_str())
            .unwrap()
            .path("/org/mpris/MediaPlayer2")
            .unwrap()
            .interface("org.mpris.MediaPlayer2.Player")
            .unwrap()
            .build()
            .await
        {
            let method = match command {
                MprisCommand::PlayPause => "PlayPause",
                MprisCommand::Next => "Next",
                MprisCommand::Previous => "Previous",
            };
            let _ = player_proxy.call_method(method, &()).await;
        }
    }
}
