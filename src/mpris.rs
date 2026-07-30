// SPDX-License-Identifier: GPL-3.0-only

//! Pure-Rust MPRIS client using zbus D-Bus proxies.
//!
//! Discovers active media players on the session bus and retrieves
//! their metadata (track title, artist) via the standard
//! `org.mpris.MediaPlayer2.Player` interface.

use crate::constants::{
    ARMED_WATCH_MAX_SLEEP_MS, ARMED_WATCH_POLL_MS, MAX_ART_FILE_BYTES, PAUSE_LEAD_US,
    SNAP_ART_PACKAGES, YOUTUBE_THUMBNAIL_URL,
};
use crate::debug_log;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use zbus::Connection;

/// The session-bus connection shared by every MPRIS call in the process.
///
/// `zbus::Connection::session()` performs a full connect + handshake on every
/// call, and the applet talks to the bus at least once a second — so the
/// connection is established once and reused. A failed attempt is not cached;
/// the next call retries.
async fn session_bus() -> Option<&'static Connection> {
    static SESSION: tokio::sync::OnceCell<Connection> = tokio::sync::OnceCell::const_new();
    SESSION.get_or_try_init(Connection::session).await.ok()
}

/// Extract a YouTube thumbnail URL from a track URL, if it's a YouTube video.
///
/// The URL is untrusted (any bus peer can publish metadata), so it goes through
/// a real URL parse — substring matching would both misfire on look-alike hosts
/// and, worse, mix byte indices between a lowercased copy and the original,
/// which can split a multi-byte character and panic. The extracted id is then
/// held to YouTube's actual id alphabet before being spliced into the
/// thumbnail URL, so nothing else can ride along in the request path.
fn youtube_thumbnail_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);

    let video_id: String = if host == "youtu.be" {
        // youtu.be/ID short links: the id is the first path segment.
        parsed.path_segments()?.next()?.to_string()
    } else if (host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com"))
        && parsed.path() == "/watch"
    {
        // youtube.com/watch?v=ID (also music., m., nocookie variants).
        parsed
            .query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.into_owned())?
    } else {
        return None;
    };

    let is_id_char = |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
    if video_id.is_empty() || !video_id.bytes().all(is_id_char) {
        return None;
    }
    Some(YOUTUBE_THUMBNAIL_URL.replace("{id}", &video_id))
}

/// Resolve a `file://` art URL to an absolute filesystem path.
///
/// Uses the URL parser rather than prefix-stripping so percent-encoding
/// (`%20` and friends) decodes correctly, dot segments are normalized, and
/// only genuinely local URLs (empty or `localhost` authority) are accepted.
fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "file" {
        return None;
    }
    parsed.to_file_path().ok()
}

/// Read a file that is expected to be album art, refusing anything that isn't
/// a bounded regular file.
///
/// The path ultimately comes from MPRIS metadata, which any process on the
/// session bus can publish — so it must be assumed hostile. Unchecked, a peer
/// could point us at `/dev/zero` (reads forever, OOM), a FIFO (the read never
/// returns and pins a blocking-pool thread on every poll), or an enormous
/// file. `O_NONBLOCK` keeps even the `open()` from blocking on a FIFO with no
/// writer, and has no effect on regular-file reads; `O_NOCTTY` prevents a tty
/// device path from being acquired as a controlling terminal. The size is
/// checked on the opened file (no stat/open race) and enforced again while
/// reading in case the file grows.
async fn read_regular_file_capped(path: &Path) -> Option<Vec<u8>> {
    use tokio::io::AsyncReadExt;

    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(path)
        .await
        .ok()?;
    let meta = file.metadata().await.ok()?;
    if !meta.is_file() || meta.len() > MAX_ART_FILE_BYTES {
        return None;
    }

    let mut bytes = Vec::new();
    // take() bounds the read even if the file grew past the cap after the
    // metadata check; landing over the cap means "too big", not "truncate".
    file.take(MAX_ART_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MAX_ART_FILE_BYTES {
        return None;
    }
    Some(bytes)
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
/// the namespace of the player that actually published the URL — never a scan
/// of every process — then a Snap-specific fallback in case AppArmor blocks
/// /proc traversal.
async fn read_art_file(
    dbus_proxy: &zbus::fdo::DBusProxy<'_>,
    bus_name: Option<&str>,
    url: &str,
) -> Option<Vec<u8>> {
    let path = file_url_to_path(url)?;

    if let Some(bytes) = read_regular_file_capped(&path).await {
        return Some(bytes);
    }

    if let Some(bus_name) = bus_name {
        if let Ok(rel) = path.strip_prefix("/") {
            if let Ok(bus) = zbus::names::BusName::try_from(bus_name) {
                if let Ok(pid) = dbus_proxy.get_connection_unix_process_id(bus).await {
                    let proc_path = PathBuf::from(format!("/proc/{pid}/root")).join(rel);
                    if let Some(bytes) = read_regular_file_capped(&proc_path).await {
                        return Some(bytes);
                    }
                }
            }
        }
    }

    let filename = path.file_name()?.to_str()?;
    for snap_name in SNAP_ART_PACKAGES {
        let snap_path =
            PathBuf::from(format!("/tmp/snap-private-tmp/snap.{snap_name}/tmp")).join(filename);
        if let Some(bytes) = read_regular_file_capped(&snap_path).await {
            return Some(bytes);
        }
    }
    None
}

/// Fallback art read for `file://` URLs, used by the UI when the bytes weren't
/// captured during the poll (e.g. switching players after startup). Same
/// lookup as the poll-time read, scoped to the player that published the URL.
pub async fn read_art_url_for_player(bus_name: Option<&str>, url: &str) -> Option<Vec<u8>> {
    let connection = session_bus().await?;
    let dbus_proxy = zbus::fdo::DBusProxy::new(connection).await.ok()?;
    read_art_file(&dbus_proxy, bus_name, url).await
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
    Pause,
    Next,
    Previous,
    /// Seek to an absolute position (microseconds) using MPRIS SetPosition.
    /// Requires the track's MPRIS object-path ID and the target position.
    SetPosition { track_id: String, position_us: i64 },
}

/// The upcoming queued track, if the player implements the optional
/// `org.mpris.MediaPlayer2.TrackList` interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedTrack {
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    pub bus_name: String,
    pub identity: String,
    pub metadata: TrackMetadata,
    pub playback_status: String,
    /// Current playback position in microseconds (0 if unknown).
    pub position_us: i64,
    /// Whether the player advertises support for skipping to the next track.
    pub can_go_next: bool,
    /// Whether the player advertises support for skipping to the previous track.
    pub can_go_previous: bool,
    /// The next track in the queue — only available for players implementing the
    /// optional TrackList interface (e.g. VLC); `None` for most players.
    pub next_track: Option<QueuedTrack>,
}

/// Read a boolean property from an MPRIS player proxy, returning `None` when the
/// property is absent or not a boolean.
async fn read_bool_property(player_proxy: &zbus::proxy::Proxy<'_>, name: &str) -> Option<bool> {
    player_proxy
        .get_property::<zbus::zvariant::OwnedValue>(name)
        .await
        .ok()
        .and_then(|v| <bool as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
}


/// Pull the four identity fields (track id, url, title, artist) out of an MPRIS
/// Metadata dict. Shared by the main poller and the armed watcher so both always
/// derive identical keys — a mismatch would fire the pause immediately.
fn metadata_identity(
    map: &HashMap<String, zbus::zvariant::OwnedValue>,
) -> (String, Option<String>, String, String) {
    let title = map
        .get("xesam:title")
        .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
        .unwrap_or_default();

    let artist = map
        .get("xesam:artist")
        .and_then(|v| {
            <Vec<String> as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone())
                .ok()
                .and_then(|artists| artists.into_iter().next())
        })
        .unwrap_or_default();

    let track_url = map
        .get("xesam:url")
        .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok());

    let track_id = map
        .get("mpris:trackid")
        .and_then(|v| {
            // Try as ObjectPath first, then as plain string.
            <zbus::zvariant::OwnedObjectPath as TryFrom<zbus::zvariant::OwnedValue>>::try_from(
                v.clone(),
            )
            .map(|op| op.to_string())
            .ok()
            .or_else(|| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
        })
        .unwrap_or_default();

    (track_id, track_url, title, artist)
}

/// A stable-ish identity for a track, used to detect when playback advances.
///
/// Combines every identity signal rather than preferring one: some players
/// (notably browsers) reuse a single `mpris:trackid` for a whole session, so
/// keying on it alone would never see a track change. MPRIS delivers Metadata as
/// one dict, so these fields change together and can't disagree mid-update.
pub fn track_key(track_id: &str, track_url: Option<&str>, title: &str, artist: &str) -> String {
    format!(
        "{track_id}\u{1f}{}\u{1f}{title}\u{1f}{artist}",
        track_url.unwrap_or("")
    )
}

/// Watch a single player and stop it just *before* it advances past `armed_key`.
///
/// Reacting to the track change is inherently a race — by the time any client
/// observes the new track, the player is already producing audio. So the primary
/// strategy is predictive: track the position and pause `PAUSE_LEAD_US` before
/// the current track ends, so the player never advances at all.
///
/// The watcher sleeps exactly as long as it needs to reach that point (capped by
/// `ARMED_WATCH_MAX_SLEEP_MS`), so it costs only a handful of D-Bus calls per
/// track rather than constant fast polling.
///
/// Detecting the track change remains as a fallback for cases prediction can't
/// cover — unknown duration (live streams), gapless/crossfaded transitions, or a
/// manual skip — in which case it pauses and rewinds the new track to its start.
///
pub async fn pause_before_next_track(bus_name: String, armed_key: String) {
    debug_log!(crate::debug::WATCH, "start bus={bus_name} armed_key={armed_key:?}");

    let Some(connection) = session_bus().await else {
        debug_log!(crate::debug::WATCH, "ABORT: no session bus");
        return;
    };
    let cmd_builder = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(connection)
        .destination(bus_name.as_str())
        .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
        .and_then(|b| b.interface("org.mpris.MediaPlayer2.Player"));
    let Ok(player) = (match cmd_builder {
        Ok(b) => b.build().await,
        Err(e) => Err(e),
    }) else {
        debug_log!(crate::debug::WATCH, "ABORT: could not build player proxy");
        return;
    };

    // Some players (notably Chromium) only refresh `Position` on discrete events
    // — a seek, a play/pause — not continuously. Polling it then returns the same
    // stale value forever and the track end is never predicted. So the position
    // is *extrapolated*: anchor on a reported value, advance it by elapsed
    // wall-clock time while playing, and re-anchor whenever the player finally
    // publishes a new reading.
    let mut last_reported: i64 = i64::MIN;
    let mut anchor_us: i64 = 0;
    let mut anchor_at = std::time::Instant::now();

    let mut tick: u64 = 0;
    loop {
        tick += 1;

        let Ok(map) = player
            .get_property::<HashMap<String, zbus::zvariant::OwnedValue>>("Metadata")
            .await
        else {
            debug_log!(crate::debug::WATCH, "#{tick} Metadata read FAILED → retry in {ARMED_WATCH_POLL_MS}ms");
            tokio::time::sleep(std::time::Duration::from_millis(ARMED_WATCH_POLL_MS)).await;
            continue;
        };

        let (track_id, track_url, title, artist) = metadata_identity(&map);
        let key = track_key(&track_id, track_url.as_deref(), &title, &artist);

        // Fallback: the track already changed before we could pre-empt it. Stop
        // and rewind so play resumes at the start of the new track.
        if key != armed_key {
            debug_log!(
            crate::debug::WATCH,
                "#{tick} FALLBACK: track changed → key={key:?} — pausing + rewinding"
            );
            let pause_res = player.call_method("Pause", &()).await;
            debug_log!(crate::debug::WATCH, "  Pause -> {:?}", pause_res.map(|_| "ok"));
            if let Ok(obj_path) = zbus::zvariant::ObjectPath::try_from(track_id.as_str()) {
                let seek_res = player.call_method("SetPosition", &(obj_path, 0i64)).await;
                debug_log!(crate::debug::WATCH, "  SetPosition(0) -> {:?}", seek_res.map(|_| "ok"));
            }
            return;
        }

        let status = player
            .get_property::<zbus::zvariant::OwnedValue>("PlaybackStatus")
            .await
            .ok()
            .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or_else(|| "Stopped".to_string());

        // Already paused/stopped by the user — nothing to pre-empt yet. Re-anchor
        // so time spent paused isn't counted as playback progress.
        if status != "Playing" {
            anchor_at = std::time::Instant::now();
            debug_log!(crate::debug::WATCH, "#{tick} status={status} (not Playing) → wait {ARMED_WATCH_MAX_SLEEP_MS}ms");
            tokio::time::sleep(std::time::Duration::from_millis(ARMED_WATCH_MAX_SLEEP_MS)).await;
            continue;
        }

        let raw_length = map.get("mpris:length");
        let length_us: i64 = raw_length
            .and_then(|v| <i64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
            .unwrap_or(0);

        // No duration to predict against (live stream): fall back to watching
        // for the track change as fast as is reasonable.
        if length_us <= 0 {
            debug_log!(
            crate::debug::WATCH,
                "#{tick} NO LENGTH (raw present={}, parsed={length_us}) → \
                 prediction impossible, polling every {ARMED_WATCH_POLL_MS}ms",
                raw_length.is_some(),
            );
            tokio::time::sleep(std::time::Duration::from_millis(ARMED_WATCH_POLL_MS)).await;
            continue;
        }

        let pos_read = player
            .get_property::<zbus::zvariant::OwnedValue>("Position")
            .await;
        let pos_ok = pos_read.is_ok();
        let reported_us: i64 = pos_read
            .ok()
            .and_then(|v| <i64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v).ok())
            .unwrap_or(0);

        // Re-anchor only when the player publishes a genuinely new value; a
        // repeated reading means it isn't tracking playback, so keep extrapolating.
        if reported_us != last_reported {
            last_reported = reported_us;
            anchor_us = reported_us;
            anchor_at = std::time::Instant::now();
        }
        let position_us = anchor_us + anchor_at.elapsed().as_micros() as i64;

        let pause_at_us = length_us - PAUSE_LEAD_US;
        let remaining_ms = (pause_at_us - position_us) / 1_000;
        debug_log!(
            crate::debug::WATCH,
            "#{tick} pos={}ms (reported={}ms) len={}ms pause_at={}ms remaining={}ms (read ok={})",
            position_us / 1_000,
            reported_us / 1_000,
            length_us / 1_000,
            pause_at_us / 1_000,
            remaining_ms,
            pos_ok,
        );

        if position_us >= pause_at_us {
            // Stop just short of the end; the player never reaches the next
            // track, so nothing of it is heard.
            debug_log!(crate::debug::WATCH, "#{tick} PREDICTIVE FIRE → pausing before end");
            let pause_res = player.call_method("Pause", &()).await;
            debug_log!(crate::debug::WATCH, "  Pause -> {:?}", pause_res.map(|_| "ok"));
            return;
        }

        // Sleep just long enough to land on the pause point.
        let wait_ms = remaining_ms.clamp(20, ARMED_WATCH_MAX_SLEEP_MS as i64);
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms as u64)).await;
    }
}

/// Look up the next queued track via the optional `TrackList` interface.
///
/// Returns `None` when the player doesn't implement TrackList (most players,
/// including browsers and Spotify), when there is no current track id to anchor
/// on, or when the current track is the last in the list.
async fn fetch_next_track(
    connection: &Connection,
    bus_name: &str,
    current_track_id: &str,
) -> Option<QueuedTrack> {
    if current_track_id.is_empty() {
        return None;
    }

    let builder = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(connection)
        .destination(bus_name)
        .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
        .and_then(|b| b.interface("org.mpris.MediaPlayer2.TrackList"))
        .ok()?;
    let track_list = builder.build().await.ok()?;

    let tracks: Vec<zbus::zvariant::OwnedObjectPath> =
        track_list.get_property("Tracks").await.ok()?;
    let idx = tracks.iter().position(|p| p.as_str() == current_track_id)?;
    let next_id = tracks.get(idx + 1)?.clone();

    let reply = track_list
        .call_method("GetTracksMetadata", &(vec![next_id],))
        .await
        .ok()?;
    let metas: Vec<HashMap<String, zbus::zvariant::OwnedValue>> =
        reply.body().deserialize().ok()?;
    let meta = metas.into_iter().next()?;

    let title = meta
        .get("xesam:title")
        .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
        .unwrap_or_default();
    let artist = meta
        .get("xesam:artist")
        .and_then(|v| {
            <Vec<String> as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone())
                .ok()
                .and_then(|a| a.into_iter().next())
        })
        .unwrap_or_default();

    if title.is_empty() && artist.is_empty() {
        return None;
    }
    Some(QueuedTrack { title, artist })
}

/// Find all active MPRIS media players and return their information.
///
/// `art_cache` maps a bus name to the (art URL, bytes) pair read on a previous
/// poll. `file://` art would otherwise be re-read from disk every second even
/// though the UI only consumes it when the URL changes; the cache keeps the
/// per-poll cost at zero for an unchanged track. Only successful reads are
/// cached — a miss retries next poll, since browsers sometimes write the art
/// file shortly after publishing the metadata.
pub async fn get_all_players(art_cache: &mut HashMap<String, (String, Vec<u8>)>) -> Vec<PlayerInfo> {
    let mut players = Vec::new();
    let Some(connection) = session_bus().await else {
        return players;
    };
    let Ok(dbus_proxy) = zbus::fdo::DBusProxy::new(connection).await else {
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

        let Ok(root_builder) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(connection)
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
            .unwrap_or_else(|| crate::fl!("unknown-player"));

        let Ok(player_builder) = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(connection)
            .destination(bus_name.as_str())
            .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
            .and_then(|b| b.interface("org.mpris.MediaPlayer2.Player"))
            else { continue; };
        let Ok(player_proxy) = player_builder.build().await else { continue; };

        let metadata = if let Ok(metadata_map) = player_proxy.get_property::<HashMap<String, zbus::zvariant::OwnedValue>>("Metadata").await {
            // Identity fields come from the shared helper so the armed watcher
            // derives byte-identical keys.
            let (track_id, track_url, title, artist) = metadata_identity(&metadata_map);

            let art_url = metadata_map
                .get("mpris:artUrl")
                .and_then(|v| <String as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok());

            // Duration in microseconds (mpris:length is int64).
            let length_us: i64 = metadata_map
                .get("mpris:length")
                .and_then(|v| <i64 as TryFrom<zbus::zvariant::OwnedValue>>::try_from(v.clone()).ok())
                .unwrap_or(0);

            // Read file:// art immediately — browsers delete the temp file shortly
            // after writing it, so a deferred async fetch always misses it. The
            // cache short-circuits the disk read while the URL stays the same.
            let art_bytes = match art_url.as_deref() {
                Some(url) if url.starts_with("file:") => {
                    match art_cache.get(&bus_name) {
                        Some((cached_url, bytes)) if cached_url == url => Some(bytes.clone()),
                        _ => {
                            let read = read_art_file(&dbus_proxy, Some(&bus_name), url).await;
                            if let Some(bytes) = &read {
                                art_cache
                                    .insert(bus_name.clone(), (url.to_string(), bytes.clone()));
                            }
                            read
                        }
                    }
                }
                _ => None,
            };

            // When file:// art is inaccessible (e.g., Flatpak sandbox) and the track
            // is a YouTube video, substitute the public thumbnail URL so the HTTP
            // fetch path in app.rs can load it without touching the filesystem.
            let art_url = if art_bytes.is_none()
                && art_url.as_deref().is_none_or(|u| u.starts_with("file:"))
            {
                track_url.as_deref()
                    .and_then(youtube_thumbnail_url)
                    .or(art_url)
            } else {
                art_url
            };

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

        // Whether the player supports skipping forward/backward. Default to true
        // when the property is missing, since the spec marks these as required.
        let can_go_next = read_bool_property(&player_proxy, "CanGoNext").await.unwrap_or(true);
        let can_go_previous =
            read_bool_property(&player_proxy, "CanGoPrevious").await.unwrap_or(true);

        // One line per player, emitted only when its state actually changes —
        // logging every poll would bury everything else at one line per second.
        // Position is deliberately excluded from the signature since it always
        // differs; the armed watcher logs position when that matters.
        if crate::debug::ENABLED {
            use std::sync::{Mutex, OnceLock};
            static LAST: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

            let signature = format!(
                "status={playback_status} title={:?} artist={:?} len={}ms \
                 can_next={can_go_next} art={:?}",
                metadata.title,
                metadata.artist,
                metadata.length_us / 1_000,
                metadata.art_url,
            );
            let seen = LAST.get_or_init(|| Mutex::new(HashMap::new()));
            let mut seen = seen.lock().unwrap_or_else(|e| e.into_inner());
            if seen.get(&bus_name) != Some(&signature) {
                debug_log!(crate::debug::MPRIS, "{bus_name}: {signature}");
                seen.insert(bus_name.clone(), signature);
            }
        }

        let next_track = fetch_next_track(connection, &bus_name, &metadata.track_id).await;

        players.push(PlayerInfo {
            bus_name,
            identity,
            metadata,
            playback_status,
            position_us,
            can_go_next,
            can_go_previous,
            next_track,
        });
    }

    // Drop cached art belonging to players that have left the bus.
    art_cache.retain(|bus, _| players.iter().any(|p| p.bus_name == *bus));

    players
}

/// Sends a command to a specific MPRIS media player.
pub async fn send_command(bus_name: String, command: MprisCommand) {
    let Some(connection) = session_bus().await else { return };
    let cmd_builder = zbus::proxy::Builder::<zbus::proxy::Proxy>::new(connection)
        .destination(bus_name.as_str())
        .and_then(|b| b.path("/org/mpris/MediaPlayer2"))
        .and_then(|b| b.interface("org.mpris.MediaPlayer2.Player"));
    let Ok(player_proxy) = (match cmd_builder {
        Ok(b) => b.build().await,
        Err(e) => Err(e),
    }) else { return };

    match command {
        MprisCommand::PlayPause => { let _ = player_proxy.call_method("PlayPause", &()).await; }
        MprisCommand::Pause     => { let _ = player_proxy.call_method("Pause", &()).await; }
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
