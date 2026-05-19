// SPDX-License-Identifier: GPL-3.0

use crate::config::{DisplayFormat, NowPlayingConfig};
use crate::fl;
use crate::mpris;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::alignment::Vertical;
use cosmic::iced::platform_specific::shell::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{window::Id, Length, Limits, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use futures_util::SinkExt;
use std::sync::LazyLock;

static AUTOSIZE_MAIN_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("autosize-main"));

/// The separator inserted between repetitions of scrolling text.
const SCROLL_GAP: &str = "    ·    ";

/// Approximate character width in pixels for estimating overflow.
const APPROX_CHAR_WIDTH: f32 = 8.0;

/// Pixels consumed by the music-note icon and its spacing, subtracted from the
/// available text area so short titles are not clipped by the container edge.
const ICON_AREA_WIDTH: f32 = 22.0; // icon 16px + row spacing 6px

/// Defines which view the popup should currently display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupState {
    MediaView,
    SettingsView,
}

impl Default for PopupState {
    fn default() -> Self {
        Self::MediaView
    }
}

/// Application model for the Now Playing panel applet.
pub struct NowPlaying {
    /// Application state managed by the COSMIC runtime.
    core: cosmic::Core,
    /// The popup window id (settings popup).
    popup: Option<Id>,
    /// Persisted configuration.
    config: NowPlayingConfig,
    /// Current track title from MPRIS.
    track_title: String,
    /// Current track artist from MPRIS.
    track_artist: String,
    /// The fully-formatted display string (formatted from title + artist).
    display_text: String,
    /// Current scroll offset in characters for the marquee effect.
    scroll_offset: usize,
    /// Whether any media player is currently active.
    has_player: bool,
    /// Currently active popup view.
    popup_state: PopupState,
    /// Current track album art URL from MPRIS.
    art_url: Option<String>,
    /// Loaded album art image for the popup.
    art_image: Option<cosmic::iced::widget::image::Handle>,
    /// List of currently active media players.
    players: Vec<mpris::PlayerInfo>,
    /// Playback status of the selected player.
    playback_status: String,
    /// Bus name of the player currently shown in the UI (may differ from config.selected_player).
    active_player_bus: Option<String>,
    /// Bus names of players that were Playing in the last MPRIS poll. Used to
    /// detect when a player newly transitions to Playing so we can auto-switch.
    last_playing_buses: Vec<String>,
    /// Live slider value while the user is dragging. Committed to config only after settling.
    slider_width: u32,
    /// Monotonically-increasing counter used to debounce width commits.
    width_settle_gen: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlayerOption {
    identity: String,
    bus_name: String,
}

impl std::fmt::Display for PlayerOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.identity)
    }
}

impl AsRef<str> for PlayerOption {
    fn as_ref(&self) -> &str {
        &self.identity
    }
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self {
            core: cosmic::Core::default(),
            popup: None,
            config: NowPlayingConfig::default(),
            track_title: String::new(),
            track_artist: String::new(),
            display_text: String::new(),
            scroll_offset: 0,
            has_player: false,
            popup_state: PopupState::default(),
            art_url: None,
            art_image: None,
            players: Vec::new(),
            playback_status: "Stopped".to_string(),
            active_player_bus: None,
            last_playing_buses: Vec::new(),
            slider_width: NowPlayingConfig::default().widget_width,
            width_settle_gen: 0,
        }
    }
}

impl NowPlaying {
    /// Rebuilds `display_text` from current track metadata and display format config.
    fn rebuild_display_text(&mut self) {
        if !self.has_player || self.track_title.is_empty() {
            self.display_text = fl!("no-media");
        } else {
            self.display_text =
                self.config
                    .display_format
                    .format(&self.track_title, &self.track_artist);
        }
        // Reset scroll when the text changes.
        self.scroll_offset = 0;
    }

    /// Returns the visible portion of the display text for the marquee effect.
    fn visible_text(&self) -> String {
        let max_chars = ((self.config.widget_width as f32 - ICON_AREA_WIDTH) / APPROX_CHAR_WIDTH) as usize;

        if self.display_text.chars().count() <= max_chars {
            // Text fits — no scrolling needed.
            return self.display_text.clone();
        }

        // Build a looping buffer: "text    ·    text    ·    "
        let looping = format!("{}{}{}", self.display_text, SCROLL_GAP, self.display_text);
        let total_chars: usize = self.display_text.chars().count() + SCROLL_GAP.chars().count();
        let offset = self.scroll_offset % total_chars;

        looping.chars().skip(offset).take(max_chars).collect()
    }

    /// Whether the text overflows and needs scrolling.
    fn needs_scroll(&self) -> bool {
        let max_chars = ((self.config.widget_width as f32 - ICON_AREA_WIDTH) / APPROX_CHAR_WIDTH) as usize;
        self.display_text.chars().count() > max_chars
    }

    /// Persist the current config to disk via cosmic-config.
    fn save_config(&self) {
        if let Ok(context) =
            cosmic_config::Config::new(
                <Self as cosmic::Application>::APP_ID,
                NowPlayingConfig::VERSION,
            )
        {
            if let Err(why) = self.config.write_entry(&context) {
                eprintln!("error saving config: {why}");
            }
        }
    }

    /// The Media controls view with album art.
    fn view_media(&self) -> Element<'_, Message> {
        let art: Element<'_, Message> = if let Some(handle) = &self.art_image {
            widget::image::viewer(handle.clone()).into()
        } else {
            widget::icon::from_name("audio-x-generic-symbolic").size(128).into()
        };

        let art_container = widget::container(art)
            .width(Length::Fixed(150.0))
            .height(Length::Fixed(150.0))
            .center_x(Length::Fill)
            .center_y(Length::Fill);

        let title = widget::text::title3(if self.track_title.is_empty() {
            fl!("no-media")
        } else {
            self.track_title.clone()
        });

        let artist = widget::text::body(self.track_artist.clone());

        let btn_prev = widget::button::icon(widget::icon::from_name("media-skip-backward-symbolic"))
            .on_press(Message::PlayerCommand(mpris::MprisCommand::Previous));
        
        let play_pause_icon = if self.playback_status == "Playing" {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        let btn_play_pause = widget::button::icon(widget::icon::from_name(play_pause_icon))
            .on_press(Message::PlayerCommand(mpris::MprisCommand::PlayPause));
            
        let btn_next = widget::button::icon(widget::icon::from_name("media-skip-forward-symbolic"))
            .on_press(Message::PlayerCommand(mpris::MprisCommand::Next));

        let controls = widget::row::with_capacity(3)
            .spacing(12)
            .push(btn_prev)
            .push(btn_play_pause)
            .push(btn_next)
            .align_y(Vertical::Center);

        let settings_btn = widget::button::icon(widget::icon::from_name("emblem-system-symbolic"))
            .on_press(Message::SwitchPopupView(PopupState::SettingsView));

        let header = if self.players.is_empty() {
            widget::row::with_capacity(2)
                .push(widget::space::horizontal().width(Length::Fill))
                .push(settings_btn)
                .align_y(Vertical::Center)
        } else {
            let options: Vec<PlayerOption> = self.players.iter().map(|p| PlayerOption { identity: p.identity.clone(), bus_name: p.bus_name.clone() }).collect();
            let selected_idx = options
                .iter()
                .position(|o| Some(&o.bus_name) == self.active_player_bus.as_ref())
                .or(Some(0));
            let picker = widget::dropdown(options.clone(), selected_idx, move |index| {
                Message::SelectPlayer(options[index].bus_name.clone())
            });
            
            widget::row::with_capacity(3)
                .push(picker)
                .push(widget::space::horizontal().width(Length::Fill))
                .push(settings_btn)
                .align_y(Vertical::Center)
        };

        let content = widget::column::with_capacity(5)
            .spacing(16)
            .padding(16)
            .align_x(cosmic::iced::alignment::Horizontal::Center)
            .push(header)
            .push(art_container)
            .push(title)
            .push(artist)
            .push(controls);

        self.core.applet.popup_container(content).into()
    }

    /// The configuration popup window: width slider, scroll speed, display format, margin.
    fn view_settings(&self) -> Element<'_, Message> {
        let back_btn = widget::button::standard("Back")
            .on_press(Message::SwitchPopupView(PopupState::MediaView));

        let width_label = widget::text::body(format!(
            "{}: {}px",
            fl!("widget-width"),
            self.slider_width
        ));
        let width_slider =
            widget::slider(100.0..=500.0, self.slider_width as f32, Message::SetWidth)
                .step(10.0);

        let speed_label = widget::text::body(format!(
            "{}: {}/10",
            fl!("scroll-speed"),
            self.config.scroll_speed
        ));
        let speed_slider = widget::slider(
            1.0..=10.0,
            self.config.scroll_speed as f32,
            |v| Message::SetScrollSpeed(v as u32),
        ).step(1.0);

        let format_label = widget::text::body(fl!("display-format"));
        let format_options: Vec<String> = vec![
            fl!("format-title-only"),
            fl!("format-artist-title"),
            fl!("format-title-artist"),
        ];
        let format_selected = Some(match self.config.display_format {
            DisplayFormat::TitleOnly => 0,
            DisplayFormat::ArtistTitle => 1,
            DisplayFormat::TitleArtist => 2,
        });
        let format_dropdown = widget::dropdown(
            format_options,
            format_selected,
            |i| Message::SetDisplayFormat(match i {
                0 => DisplayFormat::TitleOnly,
                1 => DisplayFormat::ArtistTitle,
                _ => DisplayFormat::TitleArtist,
            }),
        );

        let margin_label = widget::text::body(format!(
            "{}: {}px",
            fl!("top-margin"),
            self.config.top_margin
        ));
        let margin_slider =
            widget::slider(-10.0..=20.0, self.config.top_margin as f32, Message::SetTopMargin)
                .step(1.0);

        let content = widget::column::with_capacity(9)
            .spacing(12)
            .padding(16)
            .push(
                widget::row::with_capacity(2)
                    .spacing(12)
                    .push(back_btn)
                    .push(widget::text::title4(fl!("app-title")))
                    .align_y(Vertical::Center)
            )
            .push(width_label)
            .push(width_slider)
            .push(margin_label)
            .push(margin_slider)
            .push(speed_label)
            .push(speed_slider)
            .push(format_label)
            .push(format_dropdown);

        self.core.applet.popup_container(content).into()
    }
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    /// Toggle the configuration popup on/off.
    TogglePopup,
    /// A popup window was closed.
    PopupClosed(Id),
    /// MPRIS metadata was updated from the background poller.
    MprisUpdate(Vec<mpris::PlayerInfo>),
    /// User selected a different media player from the dropdown.
    SelectPlayer(String),
    /// Switch between popup views.
    SwitchPopupView(PopupState),
    /// Fetch album art asynchronously.
    FetchAlbumArt(String),
    /// Album art fetch completed.
    AlbumArtLoaded(Option<cosmic::iced::widget::image::Handle>),
    /// Send a command to the MPRIS player.
    PlayerCommand(crate::mpris::MprisCommand),
    /// Scroll timer tick — advance the marquee offset.
    ScrollTick,
    /// User changed the widget width via the slider (live, while dragging).
    SetWidth(f32),
    /// Fired after the width slider settles; commits the width to config.
    WidthSettled(u64),
    /// User changed the scroll speed (1 = slowest, 10 = fastest).
    SetScrollSpeed(u32),
    /// User changed the display format.
    SetDisplayFormat(DisplayFormat),
    /// User changed the top margin.
    SetTopMargin(f32),
    /// Configuration was changed externally (e.g. another instance or file edit).
    ConfigChanged(NowPlayingConfig),
}

/// Helper: creates the MPRIS poller stream.
fn mpris_poller_stream(_data: &u8) -> impl cosmic::iced::futures::Stream<Item = Message> {
    cosmic::iced::stream::channel(4, async |mut channel: cosmic::iced::futures::channel::mpsc::Sender<Message>| {
        loop {
            let players = mpris::get_all_players().await;
            _ = channel.send(Message::MprisUpdate(players)).await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    })
}

/// Helper: creates the scroll timer stream based on the scroll speed level (1-10).
fn scroll_timer_stream(speed: &u32) -> impl cosmic::iced::futures::Stream<Item = Message> {
    // level 1 = 300 ms/tick (slowest), level 10 = 30 ms/tick (fastest)
    let tick_ms = (330u64).saturating_sub(*speed as u64 * 30).max(30);
    cosmic::iced::stream::channel(4, async move |mut channel: cosmic::iced::futures::channel::mpsc::Sender<Message>| {
        let tick_duration = std::time::Duration::from_millis(tick_ms);
        loop {
            tokio::time::sleep(tick_duration).await;
            _ = channel.send(Message::ScrollTick).await;
        }
    })
}

impl cosmic::Application for NowPlaying {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "com.github.cosmic_media_now_playing_applet";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initialize the applet: load persisted config, set default state.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let config = cosmic_config::Config::new(Self::APP_ID, NowPlayingConfig::VERSION)
            .map(|context| match NowPlayingConfig::get_entry(&context) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        let mut app = NowPlaying {
            core,
            config,
            ..Default::default()
        };
        app.slider_width = app.config.widget_width;
        app.rebuild_display_text();

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// The panel view: music icon + scrolling track text.
    ///
    /// Uses `autosize` to communicate the desired surface size to the panel
    /// compositor, which is the mechanism that actually controls the applet's
    /// width on the panel bar.
    fn view(&self) -> Element<'_, Self::Message> {
        let panel_height = self.core.applet.suggested_size(true).1
            + 2 * self.core.applet.suggested_padding(true).1;

        if !self.has_player || self.track_title.is_empty() {
            return widget::autosize::autosize(
                widget::Space::new().width(Length::Fixed(0.0)).height(Length::Fixed(0.0)),
                AUTOSIZE_MAIN_ID.clone(),
            ).into();
        }

        let icon = widget::icon::from_name("audio-x-generic-symbolic").size(16);
        let text = widget::text::body(self.visible_text())
            .wrapping(cosmic::iced::widget::text::Wrapping::None);

        let content = widget::row::with_capacity(2)
            .push(icon)
            .push(
                widget::container(text)
                    .width(Length::Fill)
                    .clip(true),
            )
            .spacing(6)
            .align_y(Vertical::Center);

        let content = widget::container(content)
            .padding([self.config.top_margin.max(0) as u16, 0, 0, 0]);

        let button = widget::button::custom(content)
            .width(Length::Fixed(self.config.widget_width as f32))
            .height(Length::Fixed(panel_height as f32))
            .on_press_down(Message::TogglePopup)
            .class(cosmic::theme::Button::AppletIcon);

        widget::autosize::autosize(button, AUTOSIZE_MAIN_ID.clone())
            .into()
    }

    /// The popup window containing either media controls or configuration settings.
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        match self.popup_state {
            PopupState::MediaView => self.view_media(),
            PopupState::SettingsView => self.view_settings(),
        }
    }

    /// Subscriptions: MPRIS poller, scroll timer, config watcher.
    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs = vec![
            // 1. MPRIS metadata poller.
            Subscription::run_with(1u8, mpris_poller_stream),
            // 2. Config watcher.
            self.watch_config::<NowPlayingConfig>(Self::APP_ID)
                .map(|update| Message::ConfigChanged(update.config)),
        ];

        // 3. Scroll timer — only active when the text overflows.
        if self.needs_scroll() {
            subs.push(Subscription::run_with(self.config.scroll_speed, scroll_timer_stream));
        }

        Subscription::batch(subs)
    }

    /// Handle messages: update state, persist config changes, manage popup.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return destroy_popup(p);
                }
                let Some(main_id) = self.core.main_window_id() else {
                    return Task::none();
                };
                let new_id = Id::unique();
                self.popup.replace(new_id);
                let mut popup_settings = self.core.applet.get_popup_settings(
                    main_id,
                    new_id,
                    None,
                    None,
                    None,
                );
                popup_settings.positioner.size_limits = Limits::NONE
                    .min_height(100.0)
                    .max_height(600.0);
                return get_popup(popup_settings);
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::MprisUpdate(players) => {
                // If the saved selection points to a player that no longer exists, clear it
                // so it doesn't interfere with auto-selection going forward.
                if let Some(ref bus) = self.config.selected_player.clone() {
                    if !players.iter().any(|p| &p.bus_name == bus) {
                        self.config.selected_player = None;
                        self.save_config();
                    }
                }

                // Auto-switch logic: prefer whichever player just transitioned to Playing.
                // Otherwise stick with the current active player as long as it's still Playing.
                // Otherwise pick any Playing player. If nothing is Playing, keep the current
                // active player (or fall back to saved selection / first player).
                let currently_playing: Vec<String> = players.iter()
                    .filter(|p| p.playback_status == "Playing")
                    .map(|p| p.bus_name.clone())
                    .collect();

                let newly_started = currently_playing.iter()
                    .find(|bus| !self.last_playing_buses.contains(bus))
                    .cloned();

                let active_player = if let Some(new_bus) = newly_started {
                    players.iter().find(|p| p.bus_name == new_bus).cloned()
                } else if !currently_playing.is_empty() {
                    self.active_player_bus.as_ref()
                        .filter(|bus| currently_playing.contains(bus))
                        .and_then(|bus| players.iter().find(|p| &p.bus_name == bus).cloned())
                        .or_else(|| players.iter().find(|p| p.playback_status == "Playing").cloned())
                } else {
                    self.active_player_bus.as_ref()
                        .and_then(|bus| players.iter().find(|p| &p.bus_name == bus).cloned())
                        .or_else(|| {
                            self.config.selected_player.as_ref()
                                .and_then(|bus| players.iter().find(|p| &p.bus_name == bus).cloned())
                        })
                        .or_else(|| players.first().cloned())
                };

                self.last_playing_buses = currently_playing;
                self.players = players;

                let (title, artist, art_url, art_bytes, status, has_player, active_bus) =
                    if let Some(p) = active_player {
                        let bus = p.bus_name.clone();
                        (p.metadata.title, p.metadata.artist, p.metadata.art_url,
                         p.metadata.art_bytes, p.playback_status, true, Some(bus))
                    } else {
                        (String::new(), String::new(), None, None, "Stopped".to_string(), false, None)
                    };

                let changed = self.track_title != title
                    || self.track_artist != artist
                    || self.has_player != has_player;

                let art_changed = self.art_url != art_url;

                self.playback_status = status;
                self.active_player_bus = active_bus;

                if changed {
                    self.track_title = title;
                    self.track_artist = artist;
                    self.has_player = has_player;
                    self.rebuild_display_text();
                }

                if art_changed {
                    self.art_url = art_url.clone();
                    self.art_image = None;
                    if let Some(bytes) = art_bytes {
                        // Bytes were read inline while the temp file still existed.
                        self.art_image = Some(cosmic::iced::widget::image::Handle::from_bytes(bytes));
                    } else if let Some(url) = art_url {
                        // For http:// and data: URLs, fetch asynchronously.
                        return Task::done(cosmic::Action::App(Message::FetchAlbumArt(url)));
                    }
                }
            }
            Message::SelectPlayer(bus_name) => {
                self.config.selected_player = Some(bus_name.clone());
                self.active_player_bus = Some(bus_name);
                self.save_config();
                
                let selected_bus = self.config.selected_player.as_ref().unwrap().clone();
                if let Some(p) = self.players.iter().find(|p| p.bus_name == selected_bus).cloned() {
                    let changed = self.track_title != p.metadata.title || self.track_artist != p.metadata.artist;
                    let art_changed = self.art_url != p.metadata.art_url;
                    
                    self.playback_status = p.playback_status.clone();
                    
                    if changed || !self.has_player {
                        self.track_title = p.metadata.title;
                        self.track_artist = p.metadata.artist;
                        self.has_player = true;
                        self.rebuild_display_text();
                    }
                    if art_changed {
                        self.art_url = p.metadata.art_url;
                        self.art_image = None;
                        if let Some(bytes) = p.metadata.art_bytes {
                            self.art_image = Some(cosmic::iced::widget::image::Handle::from_bytes(bytes));
                        } else if let Some(url) = self.art_url.clone() {
                            return Task::done(cosmic::Action::App(Message::FetchAlbumArt(url)));
                        }
                    }
                }
            }
            Message::ScrollTick => {
                if self.needs_scroll() {
                    let total_chars =
                        self.display_text.chars().count() + SCROLL_GAP.chars().count();
                    self.scroll_offset = (self.scroll_offset + 1) % total_chars;
                }
            }
            Message::SetWidth(w) => {
                self.slider_width = w as u32;
                self.width_settle_gen += 1;
                let gen = self.width_settle_gen;
                return Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                        gen
                    },
                    |gen| cosmic::Action::App(Message::WidthSettled(gen)),
                );
            }
            Message::WidthSettled(gen) => {
                if gen == self.width_settle_gen {
                    self.config.widget_width = self.slider_width;
                    self.save_config();
                    self.scroll_offset = 0;
                }
            }
            Message::SetScrollSpeed(level) => {
                self.config.scroll_speed = level.clamp(1, 10);
                self.save_config();
            }
            Message::SetDisplayFormat(format) => {
                self.config.display_format = format;
                self.rebuild_display_text();
                self.save_config();
            }
            Message::SetTopMargin(m) => {
                self.config.top_margin = m as i32;
                self.save_config();
            }
            Message::SwitchPopupView(state) => {
                self.popup_state = state;
            }
            Message::FetchAlbumArt(url) => {
                return Task::perform(fetch_album_art(url), |h| cosmic::Action::App(h));
            }
            Message::AlbumArtLoaded(handle) => {
                self.art_image = handle;
            }
            Message::PlayerCommand(cmd) => {
                let target_bus = self.active_player_bus.clone()
                    .or_else(|| self.players.first().map(|p| p.bus_name.clone()));
                if let Some(bus) = target_bus {
                    return Task::perform(crate::mpris::send_command(bus, cmd), |_| cosmic::Action::App(Message::ScrollTick));
                }
            }
            Message::ConfigChanged(config) => {
                if self.config != config {
                    self.config = config;
                    self.slider_width = self.config.widget_width;
                    self.rebuild_display_text();
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

/// Helper: Fetches album art from a local file path, remote HTTP URL, or inline data URI.
///
/// Firefox (and Chromium) expose MPRIS artwork as data: URIs with base64-encoded image
/// data rather than file:// or https:// URLs, so all three schemes must be handled.
async fn fetch_album_art(url: String) -> Message {
    Message::AlbumArtLoaded(fetch_art_bytes(&url).await.map(cosmic::iced::widget::image::Handle::from_bytes))
}

async fn fetch_art_bytes(url: &str) -> Option<Vec<u8>> {
    if url.starts_with("file://") {
        let path = url.strip_prefix("file://")?;
        if let Ok(bytes) = tokio::fs::read(path).await {
            return Some(bytes);
        }
        // Scan every /proc/<pid>/root/<path> to find the file inside any sandbox.
        // This is a last-resort for when the inline read in get_all_players() missed it.
        if let Ok(mut proc_entries) = tokio::fs::read_dir("/proc").await {
            while let Ok(Some(entry)) = proc_entries.next_entry().await {
                let pid_str = entry.file_name();
                let pid_str = pid_str.to_string_lossy();
                if !pid_str.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let proc_path = format!("/proc/{pid_str}/root{path}");
                if let Ok(bytes) = tokio::fs::read(&proc_path).await {
                    return Some(bytes);
                }
            }
        }
        let filename = std::path::Path::new(path).file_name()?.to_str()?;
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
    } else if url.starts_with("data:") {
        // Format: data:[<mediatype>][;base64],<data>
        let comma = url.find(',')?;
        let header = &url["data:".len()..comma];
        let data = &url[comma + 1..];
        if header.ends_with(";base64") {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(data.trim()).ok()
        } else {
            None
        }
    } else if url.starts_with("http") {
        eprintln!("[art] http fetch: {url}");
        let client = match reqwest::Client::builder()
            .user_agent("cosmic-media-now-playing-applet/0.1")
            .build()
        {
            Ok(c) => c,
            Err(e) => { eprintln!("[art] client build error: {e}"); return None; }
        };
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => { eprintln!("[art] http send error: {e}"); return None; }
        };
        let status = resp.status();
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => { eprintln!("[art] http body error: {e}"); return None; }
        };
        eprintln!("[art] http {status} → {} bytes", bytes.len());
        Some(bytes.to_vec())
    } else {
        eprintln!("[art] unsupported url scheme: {url}");
        None
    }
}
