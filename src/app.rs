// SPDX-License-Identifier: GPL-3.0

use crate::config::{DisplayFormat, NowPlayingConfig, ScrollSpeed};
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
        let max_chars = (self.config.widget_width as f32 / APPROX_CHAR_WIDTH) as usize;

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
        let max_chars = (self.config.widget_width as f32 / APPROX_CHAR_WIDTH) as usize;
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
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    /// Toggle the configuration popup on/off.
    TogglePopup,
    /// A popup window was closed.
    PopupClosed(Id),
    /// MPRIS metadata was updated from the background poller.
    MprisUpdate {
        title: String,
        artist: String,
        has_player: bool,
    },
    /// Scroll timer tick — advance the marquee offset.
    ScrollTick,
    /// User changed the widget width via the slider.
    SetWidth(f32),
    /// User changed the scroll speed.
    SetScrollSpeed(ScrollSpeed),
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
            let msg = match mpris::get_active_track().await {
                Some(meta) => Message::MprisUpdate {
                    title: meta.title,
                    artist: meta.artist,
                    has_player: true,
                },
                None => Message::MprisUpdate {
                    title: String::new(),
                    artist: String::new(),
                    has_player: false,
                },
            };

            _ = channel.send(msg).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    })
}

/// Helper: creates the scroll timer stream based on the scroll speed.
fn scroll_timer_stream(speed: &ScrollSpeed) -> impl cosmic::iced::futures::Stream<Item = Message> {
    let tick_ms = speed.tick_ms();
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
        if !self.has_player {
            return widget::autosize::autosize(
                widget::space::horizontal().width(Length::Fixed(0.0)),
                AUTOSIZE_MAIN_ID.clone(),
            )
            .into();
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

        // Apply the configurable top margin to shift text vertically.
        let content = widget::container(content)
            .padding([self.config.top_margin.max(0) as u16, 0, 0, 0]);

        // Use the panel-height spacer to ensure the surface is tall enough,
        // and set the total width to our configured widget_width.
        let panel_height = self.core.applet.suggested_size(true).1
            + 2 * self.core.applet.suggested_padding(true).1;

        let button = widget::button::custom(content)
            .width(Length::Fixed(self.config.widget_width as f32))
            .height(Length::Fixed(panel_height as f32))
            .on_press_down(Message::TogglePopup)
            .class(cosmic::theme::Button::AppletIcon);

        widget::autosize::autosize(button, AUTOSIZE_MAIN_ID.clone())
            .into()
    }

    /// The configuration popup window: width slider, scroll speed, display format.
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let width_label = widget::text::body(format!(
            "{}: {}px",
            fl!("widget-width"),
            self.config.widget_width
        ));
        let width_slider =
            widget::slider(100.0..=500.0, self.config.widget_width as f32, Message::SetWidth)
                .step(10.0);

        let speed_label = widget::text::body(fl!("scroll-speed"));
        let speed_row = widget::row::with_capacity(3)
            .spacing(4)
            .push(
                widget::button::standard(fl!("speed-slow"))
                    .on_press(Message::SetScrollSpeed(ScrollSpeed::Slow))
                    .class(if self.config.scroll_speed == ScrollSpeed::Slow {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    }),
            )
            .push(
                widget::button::standard(fl!("speed-medium"))
                    .on_press(Message::SetScrollSpeed(ScrollSpeed::Medium))
                    .class(if self.config.scroll_speed == ScrollSpeed::Medium {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    }),
            )
            .push(
                widget::button::standard(fl!("speed-fast"))
                    .on_press(Message::SetScrollSpeed(ScrollSpeed::Fast))
                    .class(if self.config.scroll_speed == ScrollSpeed::Fast {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    }),
            );

        let format_label = widget::text::body(fl!("display-format"));
        let format_col = widget::column::with_capacity(3)
            .spacing(4)
            .push(
                widget::button::standard(fl!("format-title-only"))
                    .on_press(Message::SetDisplayFormat(DisplayFormat::TitleOnly))
                    .class(if self.config.display_format == DisplayFormat::TitleOnly {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    })
                    .width(Length::Fill),
            )
            .push(
                widget::button::standard(fl!("format-artist-title"))
                    .on_press(Message::SetDisplayFormat(DisplayFormat::ArtistTitle))
                    .class(if self.config.display_format == DisplayFormat::ArtistTitle {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    })
                    .width(Length::Fill),
            )
            .push(
                widget::button::standard(fl!("format-title-artist"))
                    .on_press(Message::SetDisplayFormat(DisplayFormat::TitleArtist))
                    .class(if self.config.display_format == DisplayFormat::TitleArtist {
                        cosmic::theme::Button::Suggested
                    } else {
                        cosmic::theme::Button::Standard
                    })
                    .width(Length::Fill),
            );

        let margin_label = widget::text::body(format!(
            "{}: {}px",
            fl!("top-margin"),
            self.config.top_margin
        ));
        let margin_slider =
            widget::slider(-10.0..=20.0, self.config.top_margin as f32, Message::SetTopMargin)
                .step(1.0);

        let content = widget::column::with_capacity(8)
            .spacing(12)
            .padding(16)
            .push(widget::text::title4(fl!("app-title")))
            .push(width_label)
            .push(width_slider)
            .push(margin_label)
            .push(margin_slider)
            .push(speed_label)
            .push(speed_row)
            .push(format_label)
            .push(format_col);

        self.core.applet.popup_container(content).into()
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
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(320.0)
                        .min_width(280.0)
                        .min_height(200.0)
                        .max_height(500.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::MprisUpdate {
                title,
                artist,
                has_player,
            } => {
                let changed = self.track_title != title
                    || self.track_artist != artist
                    || self.has_player != has_player;
                if changed {
                    self.track_title = title;
                    self.track_artist = artist;
                    self.has_player = has_player;
                    self.rebuild_display_text();
                }
            }
            Message::ScrollTick => {
                if self.needs_scroll() {
                    self.scroll_offset += 1;
                }
            }
            Message::SetWidth(w) => {
                self.config.widget_width = w as u32;
                self.save_config();
                self.scroll_offset = 0;
            }
            Message::SetScrollSpeed(speed) => {
                self.config.scroll_speed = speed;
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
            Message::ConfigChanged(config) => {
                if self.config != config {
                    self.config = config;
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
