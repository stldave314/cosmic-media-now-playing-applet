// SPDX-License-Identifier: GPL-3.0

//! Pure-Rust MPRIS client using zbus D-Bus proxies.
//!
//! Discovers active media players on the session bus and retrieves
//! their metadata (track title, artist) via the standard
//! `org.mpris.MediaPlayer2.Player` interface.

use std::collections::HashMap;
use std::path::Path;
use zbus::Connection;

/// Extract a YouTube thumbnail URL from a track URL, if it's a YouTube video.
fn youtube_thumbnail_url(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    // youtube.com/watch?v=ID (also matches www., music., m., youtube-nocookie.com)
    if lower.contains("youtube.com/watch") || lower.contains("youtube-nocookie.com/watch") {
        if let Some(query) = url.split('?').nth(1) {
            for param in query.split('&') {
                if let Some(video_id) = param.strip_prefix("v=") {
                    let video_id = video_id.split('&').next().unwrap_or(video_id);
                    if !video_id.is_empty() {
                        return Some(format!("https://img.youtube.com/vi/{video_id}/hqdefault.jpg"));
                    }
                }
            }
        }
    }
    // youtu.be/ID short links
    if let Some(idx) = lower.find("youtu.be/") {
        let path = &url[idx + "youtu.be/".len()..];
        let video_id = path.split(&['?', '&', '#'][..]).next()?;
        if !video_id.is_empty() {
            return Some(format!("https://img.youtube.com/vi/{video_id}/hqdefault.jpg"));
        }
    }
    None
}

/// Read a media player's art file from any sandbox by going through the player's
/// mount namespace via `/proc/<pid>/root`.
///
/// Browsers and other sandboxed players (Flatpak, Snap, bwrap, custom containers)
/// often report file:// art URLs that point inside their private filesystem
/// namespace — paths that don't exist on the host. Linux exposes each process's
/// view of the filesystem at `/proc/<pid>/root`, so reading
/// `/proc/<player_pid>/root/<path>` gives us the file even when the bare host
/// path does not. This works uniformly for every sandbox technology.
///
/// We try the literal path first (native packages, unsandboxed players), then
/// the per-process namespace, then a Snap-specific fallback in case AppArmor
/// blocks /proc traversal.
async fn read_art_file(
    dbus_proxy: &zbus::fdo::DBusProxy<'_>,
    bus_name: &str,
    path: &str,
) -> Option<Vec<u8>> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        return Some(bytes);
    }

    if let Ok(bus) = zbus::names::BusName::try_from(bus_name) {
        if let Ok(pid) = dbus_proxy.get_connection_unix_process_id(bus).await {
            let proc_path = format!("/proc/{pid}/root{path}");
            if let Ok(bytes) = tokio::fs::read(&proc_path).await {
                return Some(bytes);
            }
        }
    }

    let filename = Path::new(path).file_name()?.to_str()?;
    for snap_name in [
        "chromium", "chromium-browser", "firefox", "spotify",
        "epiphany", "brave", "vivaldi", "opera",
    ] {
        let snap_path = format!("/tmp/snap-private-tmp/snap.{snap_name}/tmp/{filename}");
        if let Ok(bytes) = tokio::fs::read(&snap_path).await {
            return Some(bytes);
        }
    }
    None
}

/// Metadata retrieved from an MPRIS player.
#[derive(Debug, Clone, Default)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub art_url: Option<String>,
    /// Pre-fetched image bytes for file:// art URLs, read immediately while the
    /// temporary file still exists (browsers delete it shortly after writing).
    pub art_bytes: Option<Vec<u8>>,
    /// Track duration in microseconds (0 if unknown).
    pub length_us: i64,
    /// MPRIS track ID object path (required by SetPosition).
    pub track_id: String,
    /// The track's canonical URL (xesam:url) — used to open the track in a browser.
    pub track_url: Option<String>,
}

impl PartialEq for TrackMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.artist == other.artist
            && self.art_url == other.art_url
            && self.length_us == other.length_us
            && self.track_id == other.track_id
    }
}
impl Eq for TrackMetadata {}

/// Commands that can be sent to the MPRIS player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MprisCommand {
    PlayPause,
    Next,
    Previous,
    /// Seek to an absolute position (microseconds) using MPRIS SetPosition.
    /// Requires the track's MPRIS object-path ID and the target position.
    SetPosition { track_id: String, position_us: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    pub bus_name: String,
    pub identity: String,
    pub metadata: TrackMetadata,
    pub playback_status: String,
    /// Current playback position in microseconds (0 if unknown).
    pub position_us: i64,
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

        let Ok(root_builder) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(&connection)
            .destination(bus_name.as_str())
            .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
            .and_then(|b| b.interface("org.mpris.MediaPlayer2"))
            else { continue; };
        let Ok(root_proxy) = root_builder.build().await else { continue; };

        let identity: String = root_proxy
            .get_property::<zbus::zvariant::OwnedValue>("Identity")
            .await
            .ok()
            .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or_else(|| "Unknown Player".to_string());

        let Ok(player_builder) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(&connection)
            .destination(bus_name.as_str())
            .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
            .and_then(|b| b.interface("org.mpris.MediaPlayer2.Player"))
            else { continue; };
        let Ok(player_proxy) = player_builder.build().await else { continue; };

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

            let track_url = metadata_map
                .get("xesam:url")
                .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok());

            // Duration in microseconds (mpris:length is int64).
            let length_us: i64 = metadata_map
                .get("mpris:length")
                .and_then(|v| <i64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
                .unwrap_or(0);

            // Track ID object path (required by SetPosition).
            let track_id: String = metadata_map
                .get("mpris:trackid")
                .and_then(|v| {
                    // Try as ObjectPath first, then as plain string.
                    <zbus::zvariant::OwnedObjectPath as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone())
                        .map(|op| op.to_string())
                        .ok()
                        .or_else(|| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
                })
                .unwrap_or_default();

            // Read file:// art immediately — browsers delete the temp file shortly
            // after writing it, so a deferred async fetch always misses it.
            let art_bytes = if let Some(path) = art_url.as_deref().and_then(|u| u.strip_prefix("file://")) {
                read_art_file(&dbus_proxy, &bus_name, path).await
            } else {
                None
            };

            // When file:// art is inaccessible (e.g., Flatpak sandbox) and the track
            // is a YouTube video, substitute the public thumbnail URL so the HTTP
            // fetch path in app.rs can load it without touching the filesystem.
            let original_art_url = art_url.clone();
            let art_url = if art_bytes.is_none()
                && art_url.as_deref().map_or(true, |u| u.starts_with("file://"))
            {
                track_url.as_deref()
                    .and_then(youtube_thumbnail_url)
                    .or(art_url)
            } else {
                art_url
            };

            eprintln!(
                "[mpris] {bus_name}: title={title:?} art_url={original_art_url:?} \
                 track_url={track_url:?} bytes={} length_us={length_us} → final art_url={art_url:?}",
                art_bytes.as_ref().map_or(0, |b| b.len()),
            );

            TrackMetadata { title, artist, art_url, art_bytes, length_us, track_id, track_url }
        } else {
            TrackMetadata::default()
        };

        let playback_status: String = player_proxy
            .get_property::<zbus::zvariant::OwnedValue>("PlaybackStatus")
            .await
            .ok()
            .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or_else(|| "Stopped".to_string());

        // Current playback position in microseconds.
        let position_us: i64 = player_proxy
            .get_property::<zbus::zvariant::OwnedValue>("Position")
            .await
            .ok()
            .and_then(|v| <i64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or(0);

        players.push(PlayerInfo {
            bus_name,
            identity,
            metadata,
            playback_status,
            position_us,
        });
    }

    players
}

/// Sends a command to a specific MPRIS media player.
pub async fn send_command(bus_name: String, command: MprisCommand) {
    let Ok(connection) = Connection::session().await else { return };
    let cmd_builder = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(&connection)
        .destination(bus_name.as_str())
        .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
        .and_then(|b| b.interface("org.mpris.MediaPlayer2.Player"));
    let Ok(player_proxy) = (match cmd_builder {
        Ok(b) => b.build().await,
        Err(e) => Err(e),
    }) else { return };

    match command {
        MprisCommand::PlayPause => { let _ = player_proxy.call_method("PlayPause", &()).await; }
        MprisCommand::Next      => { let _ = player_proxy.call_method("Next", &()).await; }
        MprisCommand::Previous  => { let _ = player_proxy.call_method("Previous", &()).await; }
        MprisCommand::SetPosition { track_id, position_us } => {
            // SetPosition(o: TrackId, x: Position)
            if let Ok(obj_path) = zbus::zvariant::ObjectPath::try_from(track_id.as_str()) {
                let _ = player_proxy.call_method("SetPosition", &(obj_path, position_us)).await;
            }
        }
    }
}
