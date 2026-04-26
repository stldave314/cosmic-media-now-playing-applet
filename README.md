<p align="center">
  <img src="resources/icon.svg" alt="Now Playing Applet Icon" width="96" height="96">
</p>

<h1 align="center">🎵 COSMIC Media Now Playing Applet</h1>

<p align="center">
  <strong>A panel applet for the <a href="https://github.com/pop-os/cosmic-epoch">COSMIC™ Desktop Environment</a> that displays the currently playing media track.</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#usage">Usage</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a>
</p>

---

## Overview

**cosmic-media-now-playing-applet** is a lightweight panel applet that integrates with any [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/)-compatible media player (Spotify, Firefox, VLC, Rhythmbox, Amberol, etc.) and displays the currently playing track directly on your COSMIC panel bar.

When the track name is too long to fit in the widget, it scrolls gracefully in a marquee-style animation. All settings — width, scroll speed, and display format — are configurable through a popup that opens when you click the applet.

---

## Features

| Feature | Description |
|---------|-------------|
| **🎵 Inline Panel Display** | Shows a music icon and track name directly on the COSMIC panel bar |
| **📜 Marquee Scrolling** | Long track titles scroll smoothly with a configurable speed |
| **📏 Configurable Width** | Adjust the widget width from 100px to 500px via a slider |
| **🎨 Display Formats** | Choose between "Title Only", "Artist — Title", or "Title — Artist" |
| **⚡ Scroll Speed** | Select Slow (80ms), Medium (50ms), or Fast (30ms) tick intervals |
| **💾 Persistent Settings** | Configuration survives restarts via COSMIC's `cosmic-config` system |
| **🔌 Universal Compatibility** | Works with **any** MPRIS-compatible media player |
| **🦀 Pure Rust** | No C library dependencies — uses `zbus` for native D-Bus communication |
| **🌐 Internationalization** | Built-in i18n support with Fluent localization |

### Supported Media Players

Any application that implements the [MPRIS D-Bus Interface](https://specifications.freedesktop.org/mpris-spec/latest/) will work, including but not limited to:

- **Spotify** (desktop app)
- **Firefox** / **Chromium** (web media)
- **VLC Media Player**
- **Amberol**
- **Rhythmbox**
- **Lollypop**
- **GNOME Music**
- **Celluloid (MPV)**
- **Audacious**
- **Clementine / Strawberry**
- **Elisa**
- **Any other MPRIS-compatible player**

---

## Installation

### Prerequisites

You need a working Rust toolchain and a few system development libraries.

#### Ubuntu / Pop!_OS / Debian

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies
sudo apt install -y \
    cargo \
    cmake \
    pkg-config \
    libexpat1-dev \
    libfontconfig-dev \
    libfreetype-dev \
    libxkbcommon-dev \
    libinput-dev \
    libgbm-dev \
    libseat-dev \
    libudev-dev
```

#### Fedora

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies
sudo dnf install -y \
    cargo \
    cmake \
    pkg-config \
    expat-devel \
    fontconfig-devel \
    freetype-devel \
    libxkbcommon-devel \
    libinput-devel \
    mesa-libgbm-devel \
    libseat-devel \
    systemd-devel
```

#### Arch Linux

```bash
sudo pacman -S --needed \
    rust \
    cmake \
    pkg-config \
    expat \
    fontconfig \
    freetype2 \
    libxkbcommon \
    libinput \
    seatd
```

### Build & Install

#### Using the install script (recommended)

```bash
git clone https://github.com/user/cosmic-media-now-playing-applet.git
cd cosmic-media-now-playing-applet

# Build and install in one step
./install.sh build-install
```

#### Using just (standard COSMIC method)

```bash
git clone https://github.com/user/cosmic-media-now-playing-applet.git
cd cosmic-media-now-playing-applet

# Build
just build-release

# Install (requires sudo)
sudo just install
```

#### Using cargo directly

```bash
git clone https://github.com/user/cosmic-media-now-playing-applet.git
cd cosmic-media-now-playing-applet

# Build
cargo build --release

# Install manually
sudo install -Dm0755 target/release/cosmic-media-now-playing-applet /usr/bin/cosmic-media-now-playing-applet
sudo install -Dm0644 resources/app.desktop /usr/share/applications/com.github.cosmic_media_now_playing_applet.desktop
sudo install -Dm0644 resources/app.metainfo.xml /usr/share/appdata/com.github.cosmic_media_now_playing_applet.metainfo.xml
sudo install -Dm0644 resources/icon.svg /usr/share/icons/hicolor/scalable/apps/com.github.cosmic_media_now_playing_applet.svg
```

### Install Script Commands

The included `install.sh` script provides several convenience commands:

| Command | Description |
|---------|-------------|
| `./install.sh build` | Build the applet in release mode |
| `./install.sh install` | Install to system (requires sudo) |
| `./install.sh build-install` | Build and install in one step |
| `./install.sh uninstall` | Remove from system (with optional config cleanup) |
| `./install.sh reinstall` | Full uninstall → rebuild → reinstall cycle |
| `./install.sh status` | Check what's currently installed |
| `./install.sh clean` | Remove build artifacts |
| `./install.sh help` | Show all available commands |

You can also set a custom install prefix:

```bash
PREFIX=/usr/local ./install.sh build-install
```

### Uninstalling

```bash
# Using install script
./install.sh uninstall

# Or using just
sudo just uninstall
```

---

## Usage

### Adding to the panel

After installing:

1. Open **COSMIC Settings** → **Desktop** → **Panel**
2. Click **Add Applet** in the panel configuration
3. Find **"Now Playing"** in the applet list
4. Add it to your desired panel position

### Testing without installing

You can run the applet in a standalone window for testing:

```bash
# From the project directory
cargo run --release

# Or using just
just run
```

> **Note:** The applet is designed to run within the COSMIC panel. Running it standalone will open a test window, but some panel-specific features (like popup positioning) may behave differently.

### What you'll see

Once running and with media playing, the applet appears on your panel:

```
♫ Artist Name — Track Title
```

- If no media is playing: **"No media playing"**
- If the text is too long: it scrolls smoothly like a marquee
- Click the applet to open the settings popup

---

## Configuration

### Settings Popup

Click the applet on the panel to open the configuration popup. All settings take effect immediately and are saved automatically.

#### Widget Width

Controls how much horizontal space the applet occupies on the panel.

- **Range:** 100px — 500px
- **Default:** 200px
- **Adjustment:** Drag the slider

#### Scroll Speed

Controls how fast long text scrolls when it overflows the widget width.

| Speed | Tick Interval | Best For |
|-------|:---:|---------|
| **Slow** | 80ms | Relaxed reading |
| **Medium** | 50ms | Balanced (default) |
| **Fast** | 30ms | Quick glancing |

#### Display Format

Controls how track metadata is formatted.

| Format | Example Output |
|--------|---------------|
| **Title Only** | `Bohemian Rhapsody` |
| **Artist — Title** | `Queen — Bohemian Rhapsody` (default) |
| **Title — Artist** | `Bohemian Rhapsody — Queen` |

> If the player only provides a title (no artist), all formats display the title alone.

### Config File Location

Settings are persisted via COSMIC's `cosmic-config` system at:

```
~/.config/cosmic/com.github.cosmic_media_now_playing_applet/v1/
```

You generally don't need to edit these files directly — use the popup instead.

---

## Architecture

### Technology Stack

| Component | Technology |
|-----------|-----------|
| **GUI Framework** | [libcosmic](https://github.com/pop-os/libcosmic) (iced-based) |
| **D-Bus Communication** | [zbus](https://crates.io/crates/zbus) v5 (pure Rust, async) |
| **Async Runtime** | [Tokio](https://tokio.rs/) |
| **Config Persistence** | [cosmic-config](https://github.com/pop-os/libcosmic) |
| **Localization** | [i18n-embed](https://crates.io/crates/i18n-embed) + [Fluent](https://projectfluent.org/) |

### How It Works

```
┌─────────────────────────────────────────────────────┐
│                    COSMIC Panel                      │
│                                                      │
│   ┌──────────────────────────────────────────────┐   │
│   │  ♫  Artist — Track Title  ←←← scrolling      │   │
│   └──────────────┬───────────────────────────────┘   │
│                  │ click                             │
│   ┌──────────────▼───────────────────────────────┐   │
│   │         Settings Popup                        │   │
│   │  ┌────────────────────────────────────────┐   │   │
│   │  │  Widget Width: [═══════●═══] 200px     │   │   │
│   │  │  Speed: [Slow] [●Medium] [Fast]        │   │   │
│   │  │  Format: ● Artist — Title              │   │   │
│   │  │          ○ Title — Artist               │   │   │
│   │  │          ○ Title Only                   │   │   │
│   │  └────────────────────────────────────────┘   │   │
│   └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘

     Background Subscriptions:
     ┌──────────────────────┐
     │  MPRIS Poller (2s)   │──→ D-Bus session bus
     │  Scroll Timer (~50ms)│──→ marquee animation
     │  Config Watcher      │──→ cosmic-config
     └──────────────────────┘
```

### Source Structure

```
cosmic-media-now-playing-applet/
├── Cargo.toml                # Dependencies & project metadata
├── justfile                  # Build/install recipes (just)
├── install.sh                # Standalone build/install script
├── i18n.toml                 # Localization configuration
├── i18n/
│   └── en/
│       └── cosmic_media_now_playing_applet.ftl   # English translations
├── resources/
│   ├── app.desktop           # Desktop entry for COSMIC panel
│   ├── app.metainfo.xml      # AppStream metadata
│   └── icon.svg              # Applet icon (music note)
└── src/
    ├── main.rs               # Entry point — i18n init + applet launch
    ├── app.rs                # Application model, view, update, subscriptions
    ├── config.rs             # Persistent configuration types
    ├── mpris.rs              # Pure-Rust MPRIS D-Bus client (zbus)
    └── i18n.rs               # Localization boilerplate
```

### Key Design Decisions

**Pure-Rust D-Bus:** Instead of using the `mpris` crate (which wraps the C `libdbus` library), this applet uses [zbus](https://crates.io/crates/zbus) — a pure-Rust, async D-Bus implementation. This eliminates all C dependencies and makes cross-compilation trivial.

**Marquee Scrolling:** Implemented via character-offset windowing. The display text is duplicated with a separator (`    ·    `), and a `scroll_offset` advances on each timer tick. The timer subscription is completely disabled when the text fits within the widget, saving CPU cycles.

**Declarative Subscriptions:** Following the iced/COSMIC architecture, all background work (MPRIS polling, scroll animation, config watching) is handled through declarative `Subscription` streams that the runtime manages automatically.

---

## Building from Source

### Debug Build

```bash
cargo build
# Binary at: target/debug/cosmic-media-now-playing-applet
```

### Release Build

```bash
cargo build --release
# Binary at: target/release/cosmic-media-now-playing-applet
```

### Linting

```bash
cargo clippy --all-features -- -W clippy::pedantic
```

### Vendored Build (offline)

```bash
# First, vendor dependencies
just vendor

# Then build offline
just build-vendored
```

---

## Troubleshooting

### "No media playing" even though music is playing

1. **Check MPRIS support:** Not all players support MPRIS. Verify with:
   ```bash
   busctl --user list | grep MediaPlayer2
   ```
   You should see entries like `org.mpris.MediaPlayer2.spotify`.

2. **Web browsers:** Firefox and Chromium expose MPRIS when playing audio/video. Make sure the media tab is active.

3. **Flatpak players:** Some Flatpak apps don't expose D-Bus correctly. Check the app's permissions.

### Applet doesn't appear in the panel applet list

Make sure the `.desktop` file is installed:
```bash
./install.sh status
```

If the desktop entry is missing, reinstall:
```bash
./install.sh reinstall
```

### Scrolling feels janky

Try adjusting the scroll speed in the settings popup. "Slow" (80ms ticks) provides the smoothest experience on lower-powered hardware.

### Build errors about missing system libraries

Install all development dependencies:
```bash
# Ubuntu/Pop!_OS
sudo apt install cmake pkg-config libexpat1-dev libfontconfig-dev libfreetype-dev libxkbcommon-dev libinput-dev libgbm-dev libseat-dev libudev-dev
```

---

## Contributing

Contributions are welcome! Here are some ways you can help:

- 🐛 **Report bugs** — Open an issue with steps to reproduce
- 💡 **Suggest features** — Open an issue with your idea
- 🔧 **Submit PRs** — Fork, branch, code, and open a pull request
- 🌐 **Add translations** — Create a new file in `i18n/<lang_code>/cosmic_media_now_playing_applet.ftl`

### Adding a Translation

1. Copy `i18n/en/cosmic_media_now_playing_applet.ftl` to `i18n/<your_lang>/`
2. Translate the strings
3. Submit a PR

Example for Spanish (`i18n/es/cosmic_media_now_playing_applet.ftl`):

```ftl
no-media = Sin reproducción
widget-width = Ancho del widget
scroll-speed = Velocidad de desplazamiento
display-format = Formato de visualización
speed-slow = Lento
speed-medium = Medio
speed-fast = Rápido
format-title-only = Solo título
format-artist-title = Artista — Título
format-title-artist = Título — Artista
app-title = Reproduciendo
```

---

## Roadmap

- [ ] Album art thumbnail in the popup
- [ ] Playback controls (play/pause, next, previous) in the popup
- [ ] Tooltip on hover with full track info
- [ ] Progress bar indicator
- [ ] Support for multiple simultaneous players
- [ ] Custom font size setting
- [ ] Click-through to focus the media player window

---

## License

This project is licensed under the **GNU General Public License v3.0** — see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- [COSMIC Desktop Environment](https://github.com/pop-os/cosmic-epoch) by System76
- [libcosmic](https://github.com/pop-os/libcosmic) — the COSMIC application framework
- [zbus](https://crates.io/crates/zbus) — pure-Rust D-Bus implementation
- [MPRIS Specification](https://specifications.freedesktop.org/mpris-spec/latest/) — the media player interface standard

---

<p align="center">
  Made with 🦀 for the COSMIC Desktop
</p>
