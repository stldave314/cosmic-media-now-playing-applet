# COSMIC Store readiness audit

**Project:** cosmic-media-now-playing-applet
**Audited state:** working tree on 2026-10-13 (HEAD `5927a73` plus uncommitted
changes: the blurred-backdrop popup, `src/backdrop.rs`, new screenshots, the
`flatpak/` directory). Crate version `0.13.0`, which is also the latest
published GitHub release (tag `v0.13.0`, 2026-07-30).
**Scope:** full code review, security audit, and suitability audit against the
COSMIC Store's distribution channel, followed by a submission runbook.

---

## 1. Verdict

**Not yet submittable.** The applet is in good technical shape: the build is
warning-free with clippy warnings denied, the tests pass, every locale is
complete, the AppStream file validates, and the MPRIS input hardening is
thorough and tested. What blocks submission is packaging and metadata, not
code:

| Severity | Count | What it means |
|---|---|---|
| **Blocking** | 4 | The Store would not list the applet, or the submission would be rejected or renamed later |
| **Should fix before submitting** | 10 | Reviewers will flag it, or it weakens the sandbox or the user experience |
| **Recommended** | 9 | Quality, maintainability, and power-use improvements; not gating |
| **Informational** | 6 | Verified as fine, recorded so nobody re-audits it |

The single most important fact learned in this audit: **the COSMIC Store
identifies applets by a `<provides><id>com.system76.CosmicApplet</id></provides>`
entry in the AppStream metadata** (see `src/main.rs` in `pop-os/cosmic-store`,
the `applet_provide` lookup). This applet's metainfo does not have it, so it
would be installed as an ordinary application and never appear under
*Applets*, and the Store's "Place applet in panel/dock" flow would not be
offered.

---

## 2. How this was verified

Everything below was run against the working tree, not inferred from reading.

| Check | Command | Result |
|---|---|---|
| Lints | `cargo clippy --all-features --locked -- -D warnings` | Clean, 0 warnings |
| Tests | `cargo test --locked` | 6 passed, 0 failed (all in `backdrop`) |
| Dependency advisories | `cargo audit` (RustSec database, 1310 advisories) | 7 vulnerabilities, 5 warnings; see §4.1 |
| Locale completeness | Script comparing every `i18n/*/*.ftl` key set and `{ $placeholders }` against `en` | 11 locales, 33 keys each, no missing, extra, duplicate, or broken placeholders |
| README locale sample | Script comparing the README `ftl` sample with `i18n/es/…` | In sync |
| AppStream | `appstreamcli validate --pedantic --no-net` on the staged file | Valid |
| AppStream (Flathub linter) | `flatpak-builder-lint appstream` | 1 warning: `screenshot-image-not-found` for `screenshot-hover.png` |
| Desktop entry | `desktop-file-validate` | 1 error: unregistered category `COSMIC` (expected, see §5.11) |
| Flatpak manifest (Flathub linter) | `flatpak-builder-lint manifest flatpak/…json` | 5 errors, 2 warnings; see §3.4 |
| Vendored crate list | Script diffing `flatpak/cargo-sources.json` against `Cargo.lock` | In sync: 633 registry crates and 12 git sources, nothing missing or stale |
| Release binary | `strings target/release/… \| grep cosmic-media-now-playing.log` | Absent (logging compiled out) |
| Linked libraries | `ldd` | libc, libm, libgcc_s, libxkbcommon only; TLS is rustls, no OpenSSL |
| Unsafe code | `grep -rn unsafe src/` | None |
| Spelling | grep for British spellings | 10 hits; see §5.5 |
| Tooling references and email addresses | grep across the tree | None in project files |
| Remote state | `git ls-remote`, `curl` on release and screenshot URLs | `v0.13.0` released; `screenshot-hover.png` returns 404 (untracked locally) |

Upstream facts were confirmed from the primary sources listed in §8, not from
memory.

---

## 3. Store suitability findings

### 3.1 Blocking: the Store cannot recognize this as an applet

`resources/app.metainfo.xml` has `<provides><binary>…</binary></provides>` but
not the applet marker. The Store's applet search (`cosmic-store/src/main.rs`)
only admits components whose `provides` contains the id
`com.system76.CosmicApplet`. The other "Now Playing" applet already in the
COSMIC Flatpak repository carries exactly this entry.

**Fix:** add, inside the existing `<provides>` block:

```xml
<id>com.system76.CosmicApplet</id>
```

### 3.2 Blocking: the application ID is not acceptable

Current ID: `com.github.cosmic_media_now_playing_applet`.

- Flathub's hard requirements reserve `com.github.` for GitHub's own projects.
  GitHub-hosted apps must use `io.github.<user>.<app>` with at least four
  components. The linter reports `appid-uses-code-hosting-domain`, and the
  Flathub docs say exceptions for this one are never granted.
- The COSMIC Flatpak repository (`pop-os/cosmic-flatpak`) is the actual
  destination for applets (its README says applets and other Flatpaks "not
  suitable for upload to Flathub" go there, and Flathub rejects software that
  requires a specific desktop environment). That repository does not enforce
  the ID rule today, but a published ID is effectively permanent: renaming
  later requires an `end-of-life.txt` rebase entry and loses every installed
  user's placement and settings. Now, before the first publication, is the
  only cheap moment to get it right.
- The ID also appears in the config path
  (`~/.config/cosmic/<APP_ID>/v1/`), so existing native-package users lose
  their settings unless migrated.

**Recommended ID:** `io.github.stldave314.cosmic_media_now_playing_applet`
(four components, matches the `developer id` already in the metainfo; last
component may use underscores). This is a decision for the maintainer; see
runbook phase 0. Every touchpoint is enumerated in runbook phase 2.

### 3.3 Blocking: display-name collision in the Store

The desktop entry and metainfo both say `Name=Now Playing`. The Store already
lists an applet named **Now Playing** (`com.github.DiegoMMR.CosmicExtAppletNowPlaying`,
from `cosmic-utils/cosmic-ext-applet-now-playing`, summary "Shows active media
in the COSMIC panel"). Two identically named applets with the same purpose
would confuse users and will be questioned in review.

**Fix:** choose a distinguishing name for the desktop entry `Name=`, the
metainfo `<name>`, the `app-title` string in every locale, and the README
title. Keep it under 20 characters, no "COSMIC" in it (trademark policy, §3.9).
Decision for the maintainer; runbook phase 0.

### 3.4 Blocking: the Flatpak manifest fails the linter and points at a stale tag

`flatpak-builder-lint manifest flatpak/com.github.cosmic_media_now_playing_applet.json`:

| Result | Code | Assessment |
|---|---|---|
| error | `appid-uses-code-hosting-domain` | Fixed by §3.2 |
| error | `finish-args-arbitrary-xdg-cache-ro-access` | Remove; see §4.2 |
| error | `finish-args-flatpak-appdata-folder-ro-access` | Remove; see §4.2 |
| error | `finish-args-only-wayland` | Correct for a panel applet; the linter grants exceptions when the app genuinely has no X11 support. Keep and explain in the PR |
| error | `finish-args-unnecessary-xdg-config-cosmic-rw-access` | Required: libcosmic reads the theme and this applet stores settings through cosmic-config under `$HOME/.config/cosmic` when `FLATPAK_ID` is set. Every COSMIC applet in the repository carries the same argument. Keep and explain in the PR |
| warning | `module-…-source-git-no-commit-with-tag` | Add a `commit` next to `tag`; Flathub treats this as an error for new submissions |
| warning | `runtime-update-available-to-org.freedesktop.Platform-26.08` | Move to `26.08` if `org.freedesktop.Sdk.Extension.rust-stable//26.08` builds the project (it is installed locally at Rust 1.98) |

The manifest's source is `tag: v0.13.0`, which predates the backdrop feature
and everything else uncommitted in the tree. `Cargo.toml` still says `0.13.0`,
the version that is already released, and this repository has no
`post-commit` hook, so the bump is manual. The `dist/*.flatpak` bundle from
2026-09-27 is stale for the same reason.

### 3.5 Should fix: screenshots and metadata quality

- `screenshot-hover.png` is referenced by URL in the metainfo but is untracked,
  so the URL is a 404 and the Flathub AppStream linter fails. Resolved by
  committing and pushing it, but the release must not be tagged before that.
- `screenshot-settings.png` is 921 KiB at 688x1272; the other three are 29 to
  169 KiB. Run it through `optipng -o7` or `pngquant` before committing.
- `<summary>` is 70 characters and starts with "A". Flathub's quality
  guidelines (which the COSMIC Store inherits for its listing cards) want at
  most 35 characters, sentence case, no leading article, no app name. Example:
  `Current track and controls on the panel` (39, still long) or
  `Now playing on your panel` (25).
- `<releases>` carries only the latest version with no `<description>`.
  Reviewers expect per-release notes. The `stage_metainfo` rewrite in
  `install.sh` only replaces the version and date, so a notes paragraph added
  by hand survives it.
- Optional but cheap: `<branding>` light and dark colors drawn from the icon's
  violet-to-blue gradient, and a `<url type="vcs-browser">`.

### 3.6 Should fix: the in-app update checker is wrong inside a Flatpak

Settings shows "Version … / Check for updates", which compares against GitHub
releases. A Store user gets updates from the Store; telling them a newer GitHub
release exists sends them to the wrong channel and is something Flathub-style
reviewers routinely ask to disable. The HTTP client, user agent, and release
URL code are fine; only the surfacing needs gating.

**Fix:** in `view_settings`, hide the update row when running sandboxed.
Detect it once with `std::env::var_os("FLATPAK_ID").is_some()` or
`Path::new("/.flatpak-info").exists()` (add a small `is_flatpak()` helper in
`constants.rs` or `app.rs`). Update the README settings section accordingly.

### 3.7 Should fix: `X-HostWaylandDisplay=true` asks for a privileged socket it does not need

`cosmic-panel` (`cosmic-panel-bin/src/space/wrapper_space.rs`) treats the
presence of this key as a request for a *privileged* compositor connection
inside a Wayland security context, and only then forwards `WAYLAND_DISPLAY`.
Upstream applets set it only when they use privileged protocols (workspaces,
tiling); the audio and notifications applets, whose shape this applet matches,
do not. This applet uses nothing beyond the ordinary panel socket.

**Fix:** delete the line from `resources/app.desktop`. Acceptance: the applet
still renders, opens its popup, and shows hover controls on a real panel.

### 3.8 Should fix: manifest permissions are broader than the feature justifies

See §4.2. Summary: drop `--filesystem=xdg-cache:ro` and
`--filesystem=~/.var/app:ro`.

### 3.9 Should fix: COSMIC trademark policy and naming

The pull-request template in `pop-os/cosmic-flatpak` requires the submitter to
have read the COSMIC Trademark Policy. Relevant points:

- "The COSMIC name should be identified as a trademark using the ™ symbol."
  The README does this and carries a no-affiliation disclaimer. Good.
- Official software uses the `cosmic-` package namespace and the
  `com.system76.` ID prefix; third parties "should avoid using these prefixes"
  and are "encouraged to use the cosmic-ext- package namespace." The crate,
  binary, and repository are all `cosmic-media-now-playing-applet`, and the
  README title is "COSMIC Media Now Playing Applet".

This wording is advisory, and the repository already contains applets that
ignore it, but it is what a System76 reviewer measures against. Recommended:
rename the crate and binary to `cosmic-ext-applet-media-now-playing` (the
repository can keep its name; GitHub redirects renamed repositories anyway,
and the update URL is derived from `Cargo.toml`), and retitle the README to
"<Display name> for the COSMIC™ desktop". Decision for the maintainer; the
rename is best done together with §3.2 since the same files change.

### 3.10 Should fix: pull-request checklist obligations

The `require-checklist` workflow closes PRs whose checkbox is not ticked. The
template asserts, among other things:

- No LLM-generated content anywhere in the PR except the applicant's own
  source and manifest. The PR description and commit message must be written
  by hand, and the manifest's comments and the metainfo text are read as part
  of the submission.
- Developer Certificate of Origin: commits need `Signed-off-by` (`git commit -s`).
- "My contribution is tested and working as described": the submitter must
  have built with `just build <id>` in a `cosmic-flatpak` checkout and tried
  the result on a COSMIC panel.

### 3.11 Informational: `Categories=COSMIC;`

`desktop-file-validate` rejects `COSMIC` as an unregistered category. Upstream
applets (`com.system76.CosmicAppletAudio.desktop` and the rest) use exactly
this value, and `appstreamcli` accepts the metainfo, so keep it. Do not add
`X-` to it.

### 3.12 Informational: config watching inside the sandbox

libcosmic is built with `dbus-config`, so at startup it tries
`com.system76.CosmicSettingsDaemon` over D-Bus for config-change
notifications and falls back to an inotify watcher when the proxy cannot be
created (`libcosmic/src/core.rs`, `watch_config`). The manifest does not grant
that name, so the fallback is what runs; it works because
`xdg-config/cosmic:rw` is granted. Either state is acceptable. Granting
`--talk-name=com.system76.CosmicSettingsDaemon` (as the clipboard-manager
applet does) would use the daemon path instead; not required.

### 3.13 Informational: opening the track URL inside the sandbox

`OpenTrackUrl` spawns `xdg-open`. The `org.freedesktop.Platform//25.08`
runtime ships `/usr/bin/xdg-open` as a portal client, so this works in the
sandbox with no extra permission. Verified by inspecting the runtime.

---

## 4. Security audit

### 4.1 Should fix: vulnerable crates in the HTTP path

`cargo audit` on `Cargo.lock`:

| Crate | Version | Advisory | Reaches the Linux binary? | Fix |
|---|---|---|---|---|
| rustls | 0.23.43 | RUSTSEC-2026-0285, TLS 1.3 handshake messages accepted across encryption levels (medium) | Yes, via reqwest for album art and the update check | `cargo update -p rustls` to ≥ 0.23.45 |
| h2 | 0.4.15 | RUSTSEC-2026-0258, unbounded empty DATA frames | Yes, via reqwest/hyper | `cargo update -p h2` to ≥ 0.4.16 |
| quinn-proto | 0.11.16 | Five advisories (two rated high) | **No.** `cargo tree --target x86_64-unknown-linux-gnu -i quinn-proto` prints nothing; it is locked for other targets only | Clears with the same `cargo update` |

Warnings (unmaintained or unsound: `paste`, `rustybuzz`, `ttf-parser`,
`lru`, `memmap2 0.8`) all arrive through libcosmic and its iced fork. They are
upstream's to fix; record them in a `deny.toml` ignore list with a comment so
CI stays green without hiding new ones.

After the update, regenerate `flatpak/cargo-sources.json` (runbook phase 4) or
the Flatpak build will fail offline.

**Add to CI** (`.github/workflows/rust.yml`): a `cargo audit` step (or
`cargo deny check advisories` with the ignore list) so a new advisory is seen
on the next push rather than the next audit.

### 4.2 Should fix: Flatpak permissions expose other apps' data

| Permission | What it actually grants | Verdict |
|---|---|---|
| `--filesystem=xdg-cache:ro` | Read access to all of `~/.cache`: browser caches, tokens, thumbnails, every app's scratch data | Remove. It exists to find album art a player cached on disk, which the applet cannot locate anyway without the player's `/proc/<pid>/root` view |
| `--filesystem=~/.var/app:ro` | Read access to the private data of every other Flatpak: browser profiles, cookies, saved passwords databases, chat histories | Remove. Same justification as above, far larger blast radius |
| `--share=network` | Outbound network | Keep: `https://` art and YouTube thumbnails |
| `--talk-name=org.mpris.MediaPlayer2.*` | Talk to any MPRIS player; the D-Bus proxy lets `ListNames` and `GetConnectionUnixProcessID` see those names | Keep; this is the minimum. Note the other Now Playing applet grants the full `--socket=session-bus`; this one correctly does not |
| `--filesystem=xdg-config/cosmic:rw` | Host COSMIC config directory | Keep (theme and settings; see §3.4) |
| `--socket=wayland` | Panel socket | Keep |

With the two filesystem grants removed, `file://` art from sandboxed players
is unreachable in the Flatpak build, exactly as the README already states;
`data:` URIs, `https://`, and the YouTube fallback still work. Update the
README permissions table to match.

### 4.3 Should fix: `install.sh flatpak-sources` executes unpinned remote code

`do_flatpak_sources` downloads `flatpak-cargo-generator.py` from the `master`
branch of `flatpak/flatpak-builder-tools` on every run and executes it. A
compromised or simply changed upstream file silently changes what ends up in
`cargo-sources.json`.

**Fix:** pin `FLATPAK_CARGO_GENERATOR_URL` to a commit SHA
(`https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/<sha>/cargo/flatpak-cargo-generator.py`),
record its SHA-256 next to it, and verify with `sha256sum -c` before running.
Bump both deliberately when updating. Also pin `aiohttp` and `tomlkit`
versions in the `pip install` line.

### 4.4 Verified: MPRIS metadata is treated as hostile, and the defenses hold

Every path fed by bus-published metadata was traced; this is the strongest
part of the codebase and should be preserved through any refactor:

- `file://` art: parsed with a real URL parser, read through `open()` with
  `O_NONBLOCK | O_NOCTTY`, regular-file check on the opened descriptor (no
  stat/open race), 20 MiB cap enforced during the read, lookups scoped to the
  publishing player's `/proc/<pid>/root` and a fixed Snap path list. A hostile
  peer can at most make the applet read a file the same user could already
  read, and the bytes never leave the process.
- `data:` URIs: `;base64` required, encoded length capped before decoding.
- `http(s)://` art: shared client with a 15 s timeout, 5-redirect cap, custom
  user agent, `Content-Length` pre-check and a streaming byte cap.
- Image decoding: header dimensions checked (≤ 4096 px per side) before any
  pixels are decoded, and `image::Limits` set on the decoder as a second
  fence. Tests cover an oversized header, a truncated body, and garbage.
- YouTube thumbnail: host allow-list and an id alphabet check, so nothing
  else can be spliced into the request path.
- Opening the track URL: `http`/`https` only, passed as a single argument,
  never through a shell, child reaped.
- `xesam:artist`, `mpris:trackid`, `xesam:title` are type-checked
  conversions with defaults, never unwrapped.
- Configuration from disk is clamped at both load and watch boundaries.
- No `unsafe` blocks. The only `libc` use is three `open(2)` flags.

### 4.5 Verified: supply chain and release hygiene

- Both workflows pin third-party actions to commit SHAs and grant the CI job
  `contents: read` only; the release job alone has `contents: write`.
- Every packaging path builds with `--locked` and the `release-build` feature,
  which forces diagnostic logging off at compile time. The release binary
  contains no trace of the log file name (verified with `strings`). Note this
  check is not discriminating while `DEVELOPER_LOGGING` is `false`; to prove
  the feature itself works, flip the constant to `true` locally once, build
  with and without `--features release-build`, and compare.
- `Cargo.toml` has no personal email; the Debian maintainer field uses the
  GitHub no-reply address.
- The debug log, when enabled, is created `0600` with `O_NOFOLLOW` under
  `$XDG_RUNTIME_DIR`.

### 4.6 Informational: D-Bus polling is a privacy non-issue but a power issue

The poller reads `Identity`, `Metadata`, `PlaybackStatus`, `Position`,
`CanGoNext`, `CanGoPrevious`, and `TrackList.Tracks` for every MPRIS name once
per second, whether or not anything changed. Nothing sensitive leaves the
machine, but on a laptop this is a steady wake-up source. See §5.1.

---

## 5. Code review

### 5.1 Recommended: replace per-second polling with MPRIS signals

`org.freedesktop.DBus.Properties.PropertiesChanged` and `Player.Seeked` are
mandatory in the MPRIS spec. Subscribe to them (zbus `receive_properties_changed`)
and to `NameOwnerChanged` for player arrival and departure, keep a slow
fallback poll (every 10 to 30 s) for players that misbehave, and the applet
goes from seven-plus round trips per player per second to near zero at rest.
The armed watcher's predictive sleep already shows the pattern. This is the
largest UX and battery win available and is not gating.

### 5.2 Recommended: stop copying album art every second

`get_all_players` clones the cached `Vec<u8>` for `file://` art into every
`PlayerInfo` on every poll, and `MprisUpdate` moves it through the channel.
With a 20 MiB cover that is a 20 MiB memcpy per second per player.
`set_art` then clones the bytes again for the image handle. Store art as
`Arc<[u8]>` (or `bytes::Bytes`) in the cache and the metadata struct; the
clones become pointer copies.

### 5.3 Low: `SelectPlayer` leaves the seek bar stale

`Message::SelectPlayer` copies title, artist, art, status, and the skip
capabilities from the chosen player, but not `length_us`, `position_us`,
`track_id`, `track_url`, or `next_track`. For up to one second after switching
players the popup shows the previous player's seek bar and times, and a seek
in that window is sent with the old track id. Extract one
`apply_player(&mpris::PlayerInfo)` used by both `MprisUpdate` and
`SelectPlayer` so the two paths cannot drift again.

### 5.4 Recommended: split `app.rs`

At 2272 lines, `src/app.rs` holds the model, both popup views, the panel
view, the update loop, the subscriptions, and the HTTP code. Natural seams:
`view/media.rs`, `view/settings.rs`, `view/panel.rs`, `update.rs`, `art.rs`
(fetching and `set_art`), `update_check.rs`. No behavior change, and the
`Palette`/`round_button`/`pill_class` helpers become reusable.

### 5.5 Should fix: British spellings

House style is American spelling in every file. Hits:

| File | Line | Word | Replace with |
|---|---|---|---|
| `src/debug.rs` | 12 | optimiser | optimizer |
| `src/config.rs` | 107 | neighbouring | neighboring |
| `src/app.rs` | 837 | cancelling | canceling |
| `resources/app.metainfo.xml` | 10, 74 | centres | centers |
| `install.sh` | 39 | centres | centers |
| `README.md` | 291 | colour | color |
| `README.md` | 393 | optimiser, categorised | optimizer, categorized |
| `README.md` | 411 | greyed | grayed |

### 5.6 Recommended: tests for the pure functions

Only `backdrop` has tests. These are cheap, deterministic, and guard the
security-relevant parsing:

- `mpris::youtube_thumbnail_url`: accepted hosts, rejected look-alikes
  (`notyoutube.com`, `youtube.com.evil`), id alphabet, `youtu.be`, missing `v`.
- `mpris::file_url_to_path`: percent-decoding, `localhost` authority, a
  non-`file` scheme, dot segments.
- `mpris::track_key`: separator cannot be forged by a title containing it
  (it can: `\u{1f}` in a title would collide; consider length-prefixing).
- `app::compare_versions`: `1.10.0 > 1.9.0`, `0.13.0 == 0.13.0`, pre-release
  suffixes.
- `config::DisplayFormat::format` and `NowPlayingConfig::clamped`.
- The `data:` URI branch of `fetch_art_bytes` (extract the parsing into a
  function first).

Make them private `#[cfg(test)]` modules; no new dependencies needed.

### 5.7 Recommended: surface command failures

`mpris::send_command` discards every result (`let _ =`). A failed `Pause`
from the armed watcher or a rejected `SetPosition` is invisible. Return the
`zbus::Result`, log it through `debug_log!(MPRIS, …)`, and `eprintln!` real
failures, matching the project's own logging rule.

### 5.8 Recommended: tool directories belong in `.git/info/exclude`

`.gitignore` line 8 ignores an editor-tool working directory. House rule is to keep tool
working directories out of committed files via `.git/info/exclude`. Remove
the line from `.gitignore` and add it to the exclude file; this also keeps
the submission PR free of tooling references.

### 5.9 Recommended: tighten `install.sh flatpak-install`

- `check_dependencies` runs before `flatpak-install`; inside the SDK both
  `cargo` and `pkg-config` exist, so it passes, but the apt/dnf hints it
  prints on failure are meaningless there. Skip the check for that command.
- `do_flatpak_install` installs only the scalable SVG icon. Add at least a
  256x256 PNG render (`rsvg-convert` at build time, or commit one) so Store
  tiles and the panel's icon cache never rasterize the SVG on the fly.

### 5.10 Informational: README accuracy

The README is current with the code (features, settings table, security
section, Flatpak permissions table). It must be updated in the same change
as §3.6, §4.2, and any rename. The "Publishing a release to the Store" steps
are correct but incomplete; replace them with a pointer to runbook phase 5.

### 5.11 Informational: packaging

`.deb` and `.rpm` metadata, the tarball, the tag-triggered release workflow,
and the `stage_metainfo` version rewrite all look correct and were exercised
by the existing `v0.13.0` release. No action.

---

## 6. Submission runbook

Each task names a level (**J** junior, **M** mid), the files involved, and
an acceptance check. Tasks within a phase are independent unless noted; the
phases are sequential. Nothing is committed until the maintainer says so;
land each phase as its own hand-written Conventional Commit.

### Phase 0: decisions (maintainer)

| # | Decision | Default recommendation |
|---|---|---|
| 0.1 | Application ID | `io.github.stldave314.cosmic_media_now_playing_applet` (§3.2) |
| 0.2 | Display name | Something other than "Now Playing", under 20 characters, no "COSMIC" (§3.3) |
| 0.3 | Crate and binary name | `cosmic-ext-applet-media-now-playing` (§3.9); repository name unchanged |
| 0.4 | Settings migration for native users | Copy `~/.config/cosmic/<old id>/v1/*` to the new path on first run if the new one is empty, then leave the old directory alone (one small function in `init`) |
| 0.5 | Runtime | `org.freedesktop.Platform//26.08` if it builds; else stay on `25.08` and note why |

### Phase 1: metadata and code fixes (J unless marked)

| # | Task | Files | Acceptance |
|---|---|---|---|
| 1.1 | Add `<id>com.system76.CosmicApplet</id>` to `<provides>` | `resources/app.metainfo.xml` | `appstreamcli validate --pedantic` still passes |
| 1.2 | Remove `X-HostWaylandDisplay=true` | `resources/app.desktop` | After `./install.sh reinstall`, applet renders, popup opens, hover controls appear |
| 1.3 | Rewrite `<summary>` (≤ 35 chars, no leading article, no name); add `<description>` notes to the current `<release>`; optional `<branding>` and `vcs-browser` URL | `resources/app.metainfo.xml` | `flatpak-builder-lint appstream` passes apart from the screenshot warning |
| 1.4 | Optimize `screenshot-settings.png`; `git add` the hover screenshot and `screenshot-cover.svg` | `resources/` | Each PNG under 200 KiB; `git status` shows them staged |
| 1.5 | Fix the ten spellings in §5.5 | listed there | The grep in §2 returns nothing |
| 1.6 (M) | Hide the version/update row inside Flatpak; document it | `src/app.rs` (`view_settings`), `src/constants.rs`, `README.md` | Row visible in `cargo run`, absent in the Flatpak build |
| 1.7 | `cargo update -p rustls -p h2`; confirm `cargo audit` shows only the libcosmic warnings; add `deny.toml` with commented ignores; add an audit step to `rust.yml` | `Cargo.lock`, `deny.toml`, `.github/workflows/rust.yml` | CI green; `cargo audit` reports 0 vulnerabilities |
| 1.8 | Drop `xdg-cache:ro` and `~/.var/app:ro` from the manifest; update the README permissions table | `flatpak/*.json`, `README.md` | Linter no longer reports the two filesystem errors |
| 1.9 (M) | Pin and checksum the cargo-sources generator; pin pip packages | `install.sh` | `./install.sh flatpak-sources` refuses a tampered download (test by editing one byte of the cached copy) |
| 1.10 | Move the tool directory on `.gitignore` line 8 to `.git/info/exclude` | `.gitignore` | `git check-ignore` on that directory still matches |
| 1.11 (M) | Fix `SelectPlayer` staleness by extracting `apply_player` | `src/app.rs` | Switching players in the popup updates the seek bar immediately |
| 1.12 | Add the unit tests in §5.6 | `src/mpris.rs`, `src/app.rs`, `src/config.rs` | `cargo test` passes; `cargo clippy -- -D warnings` clean |

### Phase 2: rename (M, one coordinated change, after phase 0)

Touchpoints for the application ID (and, if chosen, the crate/binary name):

| File | What changes |
|---|---|
| `Cargo.toml` | `name`, the four asset paths in `[package.metadata.deb]` and `[package.metadata.generate-rpm]` |
| `src/app.rs` | `const APP_ID` |
| `src/main.rs` | nothing unless the crate name changes (module paths are unaffected) |
| `resources/app.desktop` | `Icon=`, `Exec=`, `Name=` |
| `resources/app.metainfo.xml` | `<id>`, `<launchable>`, `<name>`, `<provides><binary>` |
| `install.sh` | `APP_NAME`, `APP_ID` |
| `justfile` | `name`, `appid` |
| `flatpak/<id>.json` | `id`, `command`, module `name`, `CARGO_HOME` path; rename the file itself |
| `i18n/*/*.ftl` | `app-title` in every locale (one change, eleven files) |
| `README.md` | title, install paths, config path, manifest path, Store section |
| `.github/workflows/release.yml` | nothing (uses `install.sh`) |
| `src/app.rs` `init` | settings migration from the old config directory (decision 0.4) |

Acceptance: `./install.sh reinstall` places the applet under the new desktop
id; `ls ~/.config/cosmic/` shows the new directory populated after changing
one setting; the §2 greps for the old id return only the migration code and
the changelog.

### Phase 3: release (M)

1. Confirm the maintainer has tested the working tree and approved the
   commits; write each commit message by hand with `git commit -s`.
2. Bump `Cargo.toml` to `0.14.0` (manual; no hook in this repository).
3. Run `cargo update`, `cargo check`, `cargo clippy --all-features -- -D warnings`,
   `cargo test` (house rule before any version change).
4. Push `main`; wait for the Rust workflow to pass.
5. Create an annotated tag whose message is the release notes:
   `git tag -a v0.14.0` then `git push origin v0.14.0`.
6. Verify: the Release workflow attaches `.deb`, `.rpm`, and tarballs for
   both architectures; `curl -I` on all four screenshot URLs returns 200.

### Phase 4: Flatpak build and local verification (M)

1. `./install.sh flatpak-sources` (now pinned) regenerates
   `flatpak/cargo-sources.json` and points the manifest at `v0.14.0`.
2. Edit the manifest: add `"commit": "<sha of v0.14.0>"` beside `tag`; set
   the runtime per decision 0.5; four-space indentation, trailing newline.
3. `flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest flatpak/<id>.json`
   Expected remaining: `finish-args-only-wayland` and the
   `xdg-config/cosmic` entry, both to be explained in the PR. Anything else
   is a regression.
4. `FLATPAK_JOBS=4 ./install.sh package-flatpak` (builds from the working
   tree, which must equal the tag at this point; keep the machine's heavy-job
   rule: nothing else building at the same time).
5. Test on a real COSMIC panel from the Flatpak install, not the native one
   (uninstall the native package first so the two cannot be confused):
   - Settings → Panel → Add applet lists the new name; it renders with the
     user's theme (confirms `xdg-config/cosmic`).
   - Play in a browser (`data:` art), Spotify or another `https://` player,
     and a YouTube tab (thumbnail fallback); confirm art in all three.
   - Change a setting, restart the panel, confirm it persisted.
   - Click the popup art for a YouTube track; the browser opens via the
     portal.
   - The settings view shows no update row.
   - `journalctl --user -b | grep <binary>` shows no repeated errors.
6. Commit the regenerated sources and manifest (`build:` prefix, hand-written
   body), push.

### Phase 5: pull request to the COSMIC Flatpak repository (maintainer's GitHub account)

1. Fork and clone `pop-os/cosmic-flatpak`; `sudo apt-get install flatpak flatpak-builder just`
   and `just ubuntu-deps` once.
2. Create `app/<id>/` containing `<id>.json` and `cargo-sources.json`,
   byte-identical to the repository's `flatpak/` copies.
3. `just build <id>`; the log lands in `log/app/<id>/x86_64.txt`. It must
   finish without network access (the CI job `build-changed` repeats exactly
   this on the PR).
4. Open the PR with a hand-written description: what the applet does, why
   it belongs here rather than Flathub (requires the COSMIC panel), the two
   linter exceptions and their reasons, and the trademark note (uses
   "for the COSMIC™ desktop", `cosmic-ext-` namespace if renamed). Tick the
   checklist box only after every line in it is true. Commits must carry
   `Signed-off-by`.
5. Respond to review on the PR itself; do not close and reopen.
6. After merge: install from the `cosmic` remote on a clean machine
   (`flatpak remote-add --if-not-exists --user cosmic https://apt.pop-os.org/cosmic/cosmic.flatpakrepo`),
   confirm it appears under *Applets* in the Store and that "Place applet"
   works.

### Phase 6: ongoing maintenance

- Every release: phase 3, then phase 4 steps 1 to 3, then a PR to
  `cosmic-flatpak` updating the tag, commit, and `cargo-sources.json`.
- Never rename the ID again without an `end-of-life.txt` rebase entry.
- Keep `<releases>` notes substantive; the Store shows them.
- Watch the CI audit step; libcosmic bumps will change the ignore list.

---

## 7. Open decisions for the maintainer

1. Application ID (§3.2) and whether to also rename the crate and binary (§3.9).
2. Display name (§3.3).
3. Settings migration strategy for existing native-package users (0.4).
4. Whether to adopt MPRIS signals now (§5.1) or after the first Store release.
   Recommendation: after, so the submission ships the code that was audited.

---

## 8. Sources consulted

- COSMIC Flatpak repository README, `justfile`, PR template, and CI workflows:
  https://github.com/pop-os/cosmic-flatpak
- COSMIC Store applet detection (`provides` lookup):
  https://github.com/pop-os/cosmic-store/blob/master/src/main.rs
- COSMIC Store category model: https://github.com/pop-os/cosmic-store/blob/master/src/nav.rs
- cosmic-panel applet launch and `X-HostWaylandDisplay` handling:
  https://github.com/pop-os/cosmic-panel/blob/master/cosmic-panel-bin/src/space/wrapper_space.rs
- Upstream applet desktop entries: https://github.com/pop-os/cosmic-applets
- libcosmic config watcher and Flatpak config paths:
  https://github.com/pop-os/libcosmic (`src/core.rs`, `cosmic-config/src/lib.rs`, `cosmic-config/src/dbus.rs`)
- COSMIC Trademark Policy: https://github.com/pop-os/cosmic-epoch/blob/master/TRADEMARK.md
- Flathub requirements: https://docs.flathub.org/docs/for-app-authors/requirements
- Flathub submission: https://docs.flathub.org/docs/for-app-authors/submission
- Flathub quality guidelines: https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines/quality-guidelines
- Flathub linter reference: https://docs.flathub.org/docs/for-app-authors/linter
- The other Now Playing applet: https://github.com/cosmic-utils/cosmic-ext-applet-now-playing
- RustSec advisories: https://rustsec.org/advisories/RUSTSEC-2026-0285, https://rustsec.org/advisories/RUSTSEC-2026-0258
