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

BIN_SRC="${CARGO_TARGET_DIR}/release/${APP_NAME}"
BIN_DST="${PREFIX}/bin/${APP_NAME}"
DESKTOP_DST="${PREFIX}/share/applications/${APP_ID}.desktop"
APPDATA_DST="${PREFIX}/share/appdata/${APP_ID}.metainfo.xml"
ICON_DST="${PREFIX}/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
CONFIG_DIR="${HOME}/.config/cosmic/${APP_ID}"

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
do_build() {
    step "Building ${APP_NAME} (release mode)..."
    cargo build --release
    success "Build complete: ${BIN_SRC}"
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
    sudo install -Dm0644 resources/app.metainfo.xml "${APPDATA_DST}"

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

    for f in "${BIN_DST}" "${DESKTOP_DST}" "${APPDATA_DST}" "${ICON_DST}"; do
        if [ -f "${f}" ]; then
            info "Removing ${f}"
            sudo rm -f "${f}"
            ((removed++))
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
    else
        warn "Nothing was installed — nothing to remove."
    fi
}

# ── Reload panel ────────────────────────────────────────────────────────────
do_reload_panel() {
    if pgrep -x cosmic-panel &>/dev/null; then
        step "Reloading COSMIC panel..."
        pkill -x cosmic-panel || true
        success "Panel restarted (cosmic-session will bring it back automatically)."
    else
        info "cosmic-panel is not running — skipping panel reload."
    fi
}

# ── Reinstall ───────────────────────────────────────────────────────────────
do_reinstall() {
    step "Reinstalling ${APP_NAME}..."
    echo ""
    do_uninstall
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
    success "Build artifacts removed."
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
    echo -e "  ${BOLD}Environment variables:${RESET}"
    echo -e "    ${YELLOW}PREFIX${RESET}          Installation prefix (default: /usr)"
    echo -e "    ${YELLOW}CARGO_TARGET_DIR${RESET}  Cargo target directory (default: target)"
    echo ""
    echo -e "  ${BOLD}Examples:${RESET}"
    echo -e "    $0 build-install        # Build and install"
    echo -e "    $0 reinstall            # Full clean reinstall"
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
