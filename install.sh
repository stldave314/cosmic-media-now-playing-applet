#!/usr/bin/env bash
# ============================================================================
# install.sh — Build, Install, Uninstall, Reinstall helper for
#              cosmic-media-now-playing-applet
# ============================================================================
set -euo pipefail

APP_NAME="cosmic-media-now-playing-applet"
APP_ID="com.github.cosmic_media_now_playing_applet"
PREFIX="${PREFIX:-/usr}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
DIST_DIR="${DIST_DIR:-dist}"

# BIN_SRC can be overridden (e.g. to install from an extracted release tarball:
# `BIN_SRC=./cosmic-media-now-playing-applet ./install.sh install`).
BIN_SRC="${BIN_SRC:-${CARGO_TARGET_DIR}/release/${APP_NAME}}"
BIN_DST="${PREFIX}/bin/${APP_NAME}"
DESKTOP_DST="${PREFIX}/share/applications/${APP_ID}.desktop"
APPDATA_DST="${PREFIX}/share/metainfo/${APP_ID}.metainfo.xml"
APPDATA_DST_LEGACY="${PREFIX}/share/appdata/${APP_ID}.metainfo.xml"
ICON_DST="${PREFIX}/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
CONFIG_DIR="${HOME}/.config/cosmic/${APP_ID}"

# Package version, read from Cargo.toml.
get_version() { grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/'; }

# Write resources/app.metainfo.xml to $1 with its newest <release> entry set to
# the version in Cargo.toml and today's date.
#
# Software centres compare that entry against what's installed to decide whether
# a package is an upgrade, so a stale value makes a newer package look like the
# installed one. Generating it keeps Cargo.toml the single source of truth
# instead of relying on the file being hand-edited each release.
stage_metainfo() {
    local dest="$1" ver today
    ver="$(get_version)"
    today="$(date +%Y-%m-%d)"
    sed -E "s|<release version=\"[^\"]*\" date=\"[^\"]*\"/>|<release version=\"${ver}\" date=\"${today}\"/>|" \
        resources/app.metainfo.xml > "${dest}"
}

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# ── Helpers ─────────────────────────────────────────────────────────────────
info()    { echo -e "${BLUE}[INFO]${RESET}    $*"; }
success() { echo -e "${GREEN}[OK]${RESET}      $*"; }
warn()    { echo -e "${YELLOW}[WARN]${RESET}    $*"; }
error()   { echo -e "${RED}[ERROR]${RESET}   $*"; }
step()    { echo -e "${CYAN}${BOLD}──▶${RESET} $*"; }

separator() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
}

# ── Dependency check ────────────────────────────────────────────────────────
check_dependencies() {
    local missing=()

    if ! command -v cargo &>/dev/null; then
        missing+=("cargo (Rust toolchain — https://rustup.rs)")
    fi

    if ! command -v pkg-config &>/dev/null; then
        missing+=("pkg-config")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        error "Missing required dependencies:"
        for dep in "${missing[@]}"; do
            echo -e "        • ${dep}"
        done
        echo ""
        info "On Ubuntu/Pop!_OS/Debian:"
        echo -e "        sudo apt install cargo cmake pkg-config libexpat1-dev libfontconfig-dev libfreetype-dev libxkbcommon-dev"
        echo ""
        info "On Fedora:"
        echo -e "        sudo dnf install cargo cmake pkg-config expat-devel fontconfig-devel freetype-devel libxkbcommon-devel"
        exit 1
    fi
}

# ── Build ───────────────────────────────────────────────────────────────────
# CARGO_FEATURES is set to "release-build" by the packaging targets, which forces
# diagnostic logging off at compile time (see src/debug.rs).
CARGO_FEATURES="${CARGO_FEATURES:-}"
# Extra cargo flags. The packaging targets add --locked so a distributable
# artifact is always built from exactly the dependency set in Cargo.lock.
CARGO_FLAGS="${CARGO_FLAGS:-}"

do_build() {
    # ${CARGO_FLAGS} is deliberately unquoted: it holds zero or more flags.
    if [ -n "${CARGO_FEATURES}" ]; then
        step "Building ${APP_NAME} (release mode, features: ${CARGO_FEATURES})..."
        # shellcheck disable=SC2086
        cargo build --release ${CARGO_FLAGS} --features "${CARGO_FEATURES}"
    else
        step "Building ${APP_NAME} (release mode)..."
        # shellcheck disable=SC2086
        cargo build --release ${CARGO_FLAGS}
    fi
    success "Build complete: ${BIN_SRC}"
}

# Everything that produces a distributable artifact goes through here, so the
# release-build feature and --locked can't be forgotten on one path.
do_release_build() {
    CARGO_FEATURES="release-build"
    CARGO_FLAGS="--locked"
    info "Release build — diagnostic logging forced OFF"
    do_build
}

# ── Install ─────────────────────────────────────────────────────────────────
do_install() {
    if [ ! -f "${BIN_SRC}" ]; then
        error "Binary not found at ${BIN_SRC}"
        info  "Run '$0 build' first, or use '$0 build-install'"
        exit 1
    fi

    step "Installing ${APP_NAME}..."

    info "Binary → ${BIN_DST}"
    sudo install -Dm0755 "${BIN_SRC}" "${BIN_DST}"

    info "Desktop entry → ${DESKTOP_DST}"
    sudo install -Dm0644 resources/app.desktop "${DESKTOP_DST}"

    info "AppStream metadata → ${APPDATA_DST}"
    stage_metainfo "${CARGO_TARGET_DIR}/app.metainfo.xml"
    sudo install -Dm0644 "${CARGO_TARGET_DIR}/app.metainfo.xml" "${APPDATA_DST}"

    info "Icon → ${ICON_DST}"
    sudo install -Dm0644 resources/icon.svg "${ICON_DST}"

    success "Installation complete!"
    echo ""
    do_reload_panel
    echo ""
    info "Add the applet to your COSMIC panel, or test with:"
    echo -e "        ${CYAN}cargo run --release${RESET}"
}

# ── Uninstall ───────────────────────────────────────────────────────────────
do_uninstall() {
    step "Uninstalling ${APP_NAME}..."
    local removed=0

    for f in "${BIN_DST}" "${DESKTOP_DST}" "${APPDATA_DST}" "${APPDATA_DST_LEGACY}" "${ICON_DST}"; do
        if [ -f "${f}" ]; then
            info "Removing ${f}"
            sudo rm -f "${f}"
            # NB: not `((removed++))` — post-increment from 0 yields an
            # arithmetic result of 0, which bash reports as exit status 1 and
            # `set -e` then treats as a fatal error, aborting mid-uninstall.
            removed=$((removed + 1))
        else
            warn "Not found (skipped): ${f}"
        fi
    done

    # Optionally remove config
    if [ -d "${CONFIG_DIR}" ]; then
        echo ""
        read -rp "$(echo -e "${YELLOW}Remove saved configuration at ${CONFIG_DIR}? [y/N]${RESET} ")" answer
        if [[ "${answer}" =~ ^[Yy]$ ]]; then
            rm -rf "${CONFIG_DIR}"
            info "Configuration removed."
        else
            info "Configuration preserved."
        fi
    fi

    if [ "${removed}" -gt 0 ]; then
        success "Uninstall complete (${removed} files removed)."
        # Removing the binary doesn't stop the running applet: the panel spawned
        # it at startup and Linux keeps the unlinked inode alive, so it stays on
        # the panel until the panel is restarted. Skipped by `reinstall`, which
        # reloads once at the end instead.
        if [ -z "${SKIP_UNINSTALL_RELOAD:-}" ]; then
            echo ""
            do_reload_panel
        fi
    else
        warn "Nothing was installed — nothing to remove."
    fi
}

# ── Reload panel ────────────────────────────────────────────────────────────
# The panel must be restarted for a new applet binary to be picked up: killing
# just the applet process does not work, because cosmic-panel spawns applets only
# at startup and does not respawn them.
#
# cosmic-session is *supposed* to bring the panel back, but it does not do so
# reliably after repeated restarts — so relaunch it here rather than assuming.
do_reload_panel() {
    if ! pgrep -x cosmic-panel &>/dev/null; then
        info "cosmic-panel is not running — skipping panel reload."
        return 0
    fi

    step "Reloading COSMIC panel..."
    pkill -x cosmic-panel || true

    # Give the session a chance to restart it on its own.
    local waited=0
    while [ "${waited}" -lt 10 ]; do
        sleep 1
        waited=$((waited + 1))
        if pgrep -x cosmic-panel &>/dev/null; then
            success "Panel restarted by cosmic-session after ${waited}s."
            return 0
        fi
    done

    # It didn't come back — start it ourselves, inheriting the session
    # environment from another COSMIC process so it can reach the compositor.
    warn "cosmic-session did not restart the panel; starting it directly..."
    local src="" proc
    for proc in cosmic-osd cosmic-bg cosmic-comp; do
        src="$(pgrep -x "${proc}" | head -1)"
        if [ -n "${src}" ]; then
            break
        fi
    done
    if [ -z "${src}" ]; then
        error "Could not find a COSMIC process to source the session environment from."
        info  "Start the panel manually with: cosmic-panel &"
        return 1
    fi

    # One array element per variable, so values containing spaces (PATH,
    # DBUS_SESSION_BUS_ADDRESS) survive intact instead of being word-split.
    local -a envvars=()
    mapfile -t envvars < <(tr '\0' '\n' < "/proc/${src}/environ" 2>/dev/null | grep -E \
        '^(WAYLAND_DISPLAY|XDG_RUNTIME_DIR|DBUS_SESSION_BUS_ADDRESS|XDG_SESSION_TYPE|XDG_CURRENT_DESKTOP|XDG_DATA_DIRS|XDG_CONFIG_DIRS|HOME|USER|PATH|DISPLAY)=' || true)

    setsid env "${envvars[@]}" nohup cosmic-panel >/dev/null 2>&1 &
    sleep 3

    if pgrep -x cosmic-panel &>/dev/null; then
        success "Panel started."
    else
        error "Panel failed to start. Run 'cosmic-panel &' from a terminal to see why."
        return 1
    fi
}

# ── Reinstall ───────────────────────────────────────────────────────────────
do_reinstall() {
    step "Reinstalling ${APP_NAME}..."
    echo ""
    # do_install reloads the panel at the end, so skip the uninstall's reload
    # rather than restarting the panel twice in one run.
    SKIP_UNINSTALL_RELOAD=1 do_uninstall
    separator
    do_build
    separator
    do_install
}

# ── Build + Install ─────────────────────────────────────────────────────────
do_build_install() {
    do_build
    separator
    do_install
}

# ── Status ──────────────────────────────────────────────────────────────────
do_status() {
    step "Installation status for ${APP_NAME}"
    echo ""

    local installed=true

    for label_file in "Binary:${BIN_DST}" "Desktop:${DESKTOP_DST}" "AppData:${APPDATA_DST}" "Icon:${ICON_DST}"; do
        local label="${label_file%%:*}"
        local file="${label_file#*:}"
        if [ -f "${file}" ]; then
            echo -e "  ${GREEN}✓${RESET}  ${label}: ${file}"
        else
            echo -e "  ${RED}✗${RESET}  ${label}: ${file} ${RED}(missing)${RESET}"
            installed=false
        fi
    done

    echo ""
    if [ -d "${CONFIG_DIR}" ]; then
        echo -e "  ${GREEN}✓${RESET}  Config: ${CONFIG_DIR}"
    else
        echo -e "  ${YELLOW}○${RESET}  Config: ${CONFIG_DIR} (not yet created)"
    fi

    echo ""
    if [ "${installed}" = true ]; then
        success "All components are installed."
    else
        warn "Some components are missing. Run '$0 build-install' to install."
    fi
}

# ── Clean ───────────────────────────────────────────────────────────────────
do_clean() {
    step "Cleaning build artifacts..."
    cargo clean
    rm -rf "${DIST_DIR}"
    success "Build artifacts removed."
}

# ── Packaging ───────────────────────────────────────────────────────────────
# Portable binary tarball: the release binary plus resources and this script,
# so it can be installed with `BIN_SRC=./<binary> ./install.sh install`.
do_package_tar() {
    do_release_build
    local ver arch name stage
    ver="$(get_version)"
    # Name by the machine actually being built on (x86_64, aarch64, …) so
    # multi-architecture releases don't collide.
    arch="$(uname -m)"
    name="${APP_NAME}-${ver}-${arch}-linux"
    stage="${DIST_DIR}/${name}"

    step "Assembling tarball..."
    mkdir -p "${stage}/resources"
    install -Dm0755 "${BIN_SRC}" "${stage}/${APP_NAME}"
    cp resources/app.desktop resources/icon.svg "${stage}/resources/"
    stage_metainfo "${stage}/resources/app.metainfo.xml"
    cp install.sh "${stage}/"
    [ -f LICENSE ] && cp LICENSE "${stage}/"
    [ -f README.md ] && cp README.md "${stage}/"
    tar -C "${DIST_DIR}" -czf "${DIST_DIR}/${name}.tar.gz" "${name}"
    rm -rf "${stage}"
    success "Created ${DIST_DIR}/${name}.tar.gz"
}

# Debian/Ubuntu/Pop!_OS package via cargo-deb.
do_package_deb() {
    if ! command -v cargo-deb &>/dev/null; then
        error "cargo-deb not found. Install it with: cargo install cargo-deb"
        return 1
    fi
    step "Building .deb package..."
    info "Release build — diagnostic logging forced OFF"
    mkdir -p "${DIST_DIR}" "${CARGO_TARGET_DIR}"
    stage_metainfo "${CARGO_TARGET_DIR}/app.metainfo.xml"
    # cargo-deb runs its own build, so the feature (and --locked) have to be
    # passed through to it.
    cargo deb -- --features release-build --locked
    cp "${CARGO_TARGET_DIR}/debian/"*.deb "${DIST_DIR}/"
    success "Created .deb in ${DIST_DIR}/"
}

# Fedora/RHEL package via cargo-generate-rpm (needs a prior release build).
do_package_rpm() {
    if ! command -v cargo-generate-rpm &>/dev/null; then
        error "cargo-generate-rpm not found. Install it with: cargo install cargo-generate-rpm"
        return 1
    fi
    do_release_build
    step "Building .rpm package..."
    mkdir -p "${DIST_DIR}" "${CARGO_TARGET_DIR}"
    stage_metainfo "${CARGO_TARGET_DIR}/app.metainfo.xml"
    cargo generate-rpm
    cp "${CARGO_TARGET_DIR}/generate-rpm/"*.rpm "${DIST_DIR}/"
    success "Created .rpm in ${DIST_DIR}/"
}

# Build every available package format into DIST_DIR (skips deb/rpm when their
# tooling isn't installed).
do_package() {
    step "Building release packages into ${DIST_DIR}/ ..."
    mkdir -p "${DIST_DIR}"
    do_package_tar
    separator
    if command -v cargo-deb &>/dev/null; then
        do_package_deb
    else
        warn "Skipping .deb — cargo-deb not installed (cargo install cargo-deb)"
    fi
    separator
    if command -v cargo-generate-rpm &>/dev/null; then
        do_package_rpm
    else
        warn "Skipping .rpm — cargo-generate-rpm not installed (cargo install cargo-generate-rpm)"
    fi
    separator
    success "Packages in ${DIST_DIR}/:"
    ls -1 "${DIST_DIR}/" 2>/dev/null | sed 's/^/        /'
}

# ── Usage ───────────────────────────────────────────────────────────────────
usage() {
    echo -e "${BOLD}${CYAN}"
    echo "  ╔══════════════════════════════════════════════════════════╗"
    echo "  ║          COSMIC Media Now Playing Applet                ║"
    echo "  ║          Build & Install Script                         ║"
    echo "  ╚══════════════════════════════════════════════════════════╝"
    echo -e "${RESET}"
    echo -e "  ${BOLD}Usage:${RESET}  $0 <command>"
    echo ""
    echo -e "  ${BOLD}Commands:${RESET}"
    echo -e "    ${GREEN}build${RESET}           Build the applet (release mode)"
    echo -e "    ${GREEN}install${RESET}         Install to system (requires sudo)"
    echo -e "    ${GREEN}build-install${RESET}   Build and install in one step"
    echo -e "    ${GREEN}uninstall${RESET}       Remove from system (requires sudo)"
    echo -e "    ${GREEN}reinstall${RESET}       Uninstall, rebuild, and reinstall"
    echo -e "    ${GREEN}status${RESET}          Check installation status"
    echo -e "    ${GREEN}clean${RESET}           Remove build artifacts"
    echo -e "    ${GREEN}help${RESET}            Show this help message"
    echo ""
    echo -e "  ${BOLD}Packaging (for releases → ${DIST_DIR}/):${RESET}"
    echo -e "    ${GREEN}package${RESET}         Build all available formats (tarball, .deb, .rpm)"
    echo -e "    ${GREEN}package-tar${RESET}     Build the portable binary tarball"
    echo -e "    ${GREEN}package-deb${RESET}     Build a .deb (needs: cargo install cargo-deb)"
    echo -e "    ${GREEN}package-rpm${RESET}     Build a .rpm (needs: cargo install cargo-generate-rpm)"
    echo ""
    echo -e "  ${BOLD}Environment variables:${RESET}"
    echo -e "    ${YELLOW}PREFIX${RESET}          Installation prefix (default: /usr)"
    echo -e "    ${YELLOW}CARGO_TARGET_DIR${RESET}  Cargo target directory (default: target)"
    echo -e "    ${YELLOW}DIST_DIR${RESET}        Package output directory (default: dist)"
    echo -e "    ${YELLOW}BIN_SRC${RESET}         Binary to install (e.g. when installing from a tarball)"
    echo ""
    echo -e "  ${BOLD}Examples:${RESET}"
    echo -e "    $0 build-install        # Build and install"
    echo -e "    $0 reinstall            # Full clean reinstall"
    echo -e "    $0 package              # Build release packages into ${DIST_DIR}/"
    echo -e "    PREFIX=/usr/local $0 build-install  # Install to /usr/local"
    echo ""
}

# ── Main ────────────────────────────────────────────────────────────────────
main() {
    # Ensure we're in the project root
    if [ ! -f "Cargo.toml" ]; then
        error "Must be run from the project root directory (where Cargo.toml is)."
        exit 1
    fi

    if [ $# -eq 0 ]; then
        usage
        exit 0
    fi

    check_dependencies

    separator
    case "${1}" in
        build)          do_build ;;
        install)        do_install ;;
        build-install)  do_build_install ;;
        uninstall)      do_uninstall ;;
        reinstall)      do_reinstall ;;
        status)         do_status ;;
        clean)          do_clean ;;
        package)        do_package ;;
        package-tar)    do_package_tar ;;
        package-deb)    do_package_deb ;;
        package-rpm)    do_package_rpm ;;
        release)        do_package ;;
        help|--help|-h) usage ;;
        *)
            error "Unknown command: ${1}"
            echo ""
            usage
            exit 1
            ;;
    esac
    separator
}

main "$@"
