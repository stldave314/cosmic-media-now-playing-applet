// SPDX-License-Identifier: GPL-3.0

mod app;
mod config;
mod i18n;
mod mpris;

fn main() -> cosmic::iced::Result {
    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Starts the applet's event loop.
    cosmic::applet::run::<app::NowPlaying>(())
}
