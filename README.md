# Wallfolio

A local-first wallpaper catalog with a Rust core, CLI, and Qt Quick desktop client.
Wallhaven and local directories provide discovery; catalog IDs belong to Wallfolio.
The daemon owns the catalog, wallpaper rotation, and thumbnail cache; GUI and CLI
use the same local API. v0.2 adds random selection, persisted rotation, duplicate
groups, Linux wallpaper backends, and an AppImage containing all three programs.
Sync, custom providers, and Windows/macOS support remain later milestones in
[ARCHITECTURE.md](ARCHITECTURE.md).

## Build

Requirements: Rust 1.89 or newer, a C++17 compiler, SQLite development libraries,
Qt 6.4+ with Quick, Quick Controls 2 and Network, CMake, and Make or Ninja.
On Arch Linux these come from `rust`, `base-devel`, `sqlite`, `qt6-base`,
`qt6-declarative`, and `cmake`. `qt6-imageformats` adds optional Qt image support.

```sh
./scripts/cargo-safe build --workspace --locked
./scripts/build-gui
```

The Rust wrapper serializes compilation and uses a 3 GiB aggregate cgroup limit,
no swap, and a CPU quota when a user systemd manager is available. Its fallback
limits each compiler process to a 3 GiB address space. Cargo also defaults to one
job and disables development/test debug symbols and incremental compilation.
The GUI wrapper builds one job at a time with the same address-space ceiling.
No language server is started by these scripts. These limits reduce OOM risk;
other applications and manually launched builds still share the laptop's memory.

CMake outputs `build/cmake-gui/wallfolio-gui`. If CMake cannot run, the wrapper
uses qmake6 and outputs `build/gui/wallfolio-gui` instead.

## Run

From the checkout, add the Rust binaries to PATH so the GUI can launch the daemon:

```sh
export PATH="$PWD/target/debug:$PATH"
build/cmake-gui/wallfolio-gui
```

Or start `wallfoliod` yourself, then use the CLI in another terminal:

```sh
wallfoliod
wallfolio provider search local /absolute/path/to/wallpapers
wallfolio provider search wallhaven "dark mountains"
wallfolio add /absolute/path/to/image.png
wallfolio add 9ok5g8 --provider wallhaven
wallfolio search
wallfolio download <catalog-id>
wallfolio favorite <catalog-id>
wallfolio tags <catalog-id> dark landscape
wallfolio search dark --favorite
wallfolio backends
wallfolio set <catalog-id> --backend swww --monitor DP-1
wallfolio random --favorite --tag dark
wallfolio rotation start --interval 1800 --favorite --tag dark
wallfolio rotation status
wallfolio rotation stop
wallfolio duplicates
wallfolio remove-local <catalog-id>
wallfolio remove <catalog-id>
```

The examples use placeholders: replace `<catalog-id>` with an ID returned by
`add` or `search`. CLI responses are JSON; errors go to stderr with a nonzero
exit status. `wallfolio completions <shell>` generates completion definitions
without requiring a running daemon. `wallfolio --help` lists all commands and pagination options.
Local discovery searches one directory (up to 10,000 entries), without recursion.
Wallhaven discovery sends only the search query and page to its public API,
without explicit filters; no account or API token is needed.

## Image lifecycle

- **Save/import** creates a catalog entry with provenance, without copying the image.
- **Download** validates supported images and streams a managed copy to
  content-addressed storage. PNG, JPEG, and WebP are supported. Downloads are
  limited to 100 MiB, 100 million pixels, 16,384 pixels per side, and a 256 MiB
  decoder allocation budget. Identical content reuses one file.
- **Delete local copy** keeps metadata, tags, and favorites. Shared managed files
  are retained until no remaining catalog item points to them.
- **Remove from library** deletes only the catalog record. Managed files remain;
  delete the local copy first if you also want to free its disk space.
- Original user files are never deleted by these operations.

The GUI supports Library, Discover, Favorites, Duplicates, preview, import/save,
download, tags, deletion, and applying wallpapers. Random and rotation controls
are in the sidebar. Generated thumbnails keep library browsing lightweight;
full local images remain available in the detail preview. Backend availability
checks the host executable and desktop/session environment, not daemon health. The selected engine is saved immediately in the local catalog and
restored when the GUI reopens. CLI `set --backend ...` also remembers the engine
after a successful apply; later applies without `--backend` use that preference.
A saved engine is not silently replaced if it is unavailable.

Discover supports entering a page number and pressing Enter or Go. Its cards and
preview controls reflect existing library, favorite, and downloaded state, including
changes made during the current visit. Downloaded images display “Downloaded”
instead of offering another download; deleting the local copy enables Download
again.

Hyprpaper uses its current `wallpaper` IPC command. Its blank monitor is a
fallback, which does not override monitors with an existing explicit wallpaper.
See the [Hyprpaper IPC documentation](https://wiki.hypr.land/Hypr-Ecosystem/hyprpaper/#ipc).

## Random, rotation, and duplicates

Random chooses an existing downloaded file from your library, with optional
favorites and exact tag filters (all tags must match). It avoids the last applied
content when another match exists. Missing local files and remote-only entries
are skipped. It never downloads a random provider result.

Rotation is opt-in, defaults to 30 minutes, and accepts intervals from 10 seconds
to seven days. The schedule, filters, and status persist in the local catalog.
The daemon runs it while the GUI is closed and resumes it after restart, applying
once if overdue instead of replaying missed intervals. A failed apply is recorded
in rotation status and retried at the next scheduled interval. Slow foreground
requests can delay a scheduled change. Login startup requires enabling the user
service or configuring your desktop to launch the AppImage daemon.

Duplicates are exact SHA-256 matches, detected when files are downloaded/imported
into managed storage. Separate catalog entries retain their own provenance,
favorites, and tags while sharing the original file. The Duplicates view and CLI
show paged groups, with counts and up to 20 members per group. There is no automatic
merging or deletion.

## Linux wallpaper engines

Wallpaper tools are optional host dependencies, including for the AppImage.

| Engine | Host command | Monitor support |
| --- | --- | --- |
| swww | `swww` with its daemon running | Output name |
| Hyprpaper | `hyprctl` with Hyprpaper running | Output name; blank means fallback |
| swaybg | `swaybg`, launched and managed by Wallfolio | Output name; blank means all |
| GNOME | `gsettings` in a GNOME session | All monitors, light and dark settings |
| KDE Plasma | `plasma-apply-wallpaperimage` | All desktops/monitors |
| Xfce | `xfconf-query` | Configured output name; all matching workspaces |
| feh | `feh` in X11 | All monitors |
| xwallpaper | `xwallpaper` in X11 | Output name |
| Nitrogen | `nitrogen` in X11 | Numeric head index; blank means all |

The GUI disables monitor entry for engines without independent monitor support.
Xfce needs existing desktop background properties; initialize these in Xfce
Settings if Wallfolio reports none. Wallfolio replaces only its own swaybg process,
retains it if replacement fails, and stops it when switching engines or when the
daemon exits. Other wallpaper processes are never terminated by Wallfolio.
Commands have bounded output and a 15-second timeout. AppImage library paths are
removed from host command environments.

Backend command references: [swaybg](https://github.com/swaywm/swaybg/blob/master/swaybg.1.scd),
[xwallpaper](https://github.com/stoeckmann/xwallpaper/blob/master/xwallpaper.1),
[feh](https://github.com/derf/feh/blob/master/man/feh.pre),
[Nitrogen](https://github.com/l3ib/nitrogen/blob/master/src/main.cc),
[GNOME](https://help.gnome.org/system-admin-guide/desktop-background.html),
[KDE](https://github.com/KDE/plasma-workspace/blob/master/wallpapers/image/plasma-apply-wallpaperimage.cpp),
[Xfce](https://docs.xfce.org/xfce/xfdesktop/usage).

## Storage and protocol

Metadata lives in `$XDG_DATA_HOME/wallfolio/wallfolio.db` (by default
`~/.local/share/wallfolio/wallfolio.db`). Images live under `originals/<hash-prefix>/`.
`wallfoliod --data-dir PATH` selects an isolated catalog and thumbnail cache.

Remote previews and generated local thumbnails use a 256 MiB cache under
`$XDG_CACHE_HOME/wallfolio/thumbnails` (default `~/.cache/wallfolio/thumbnails`).
A single worker decodes one image at a time, with at most 100 queued jobs. Cached
images persist across restarts; oldest-accessed entries are evicted. Preview
failures do not fail discovery, and the GUI falls back if an entry is evicted.

The socket is `$XDG_RUNTIME_DIR/wallfolio/wallfoliod.sock`, falling back to
`~/.cache/wallfolio/wallfoliod.sock`. `WALLFOLIO_SOCKET` overrides it for both
clients and the daemon; CLI and daemon also accept `--socket PATH`.
Only one daemon may own a catalog or socket. Requests are serialized, including
network work; a slow download can delay other clients until its timeout.

See [the protocol reference](docs/protocol.md) and
[implementation status](docs/implementation-status.md).

## Verify

```sh
./scripts/cargo-safe test --workspace --locked
./scripts/cargo-safe clippy --workspace --all-targets --locked -- -D warnings
./scripts/cargo-safe build --workspace --locked
python3 tests/smoke.py
python3 tests/backends.py
WALLFOLIO_TEST_GUI=build/cmake-gui/wallfolio-gui python3 tests/smoke.py
```

GUI interaction tests (Qt's qmltestrunner executable may be under `/usr/lib/qt6/bin`):

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic /usr/lib/qt6/bin/qmltestrunner -input tests/qml
```

An optional network test exercises the real Wallhaven adapter and downloads one
image into a temporary directory, which is cleaned afterward:

```sh
python3 tests/live_provider.py
```

The smoke test uses a temporary catalog, local PNG fixtures, and a fake wallpaper
setter. It checks persistence, provenance IDs, downloads, shared-file deletion,
favorites/tags, protocol failures, daemon exclusivity, and backend arguments.
The GUI variant renders a populated library offscreen and rejects runtime QML
errors. It writes `build/library-preview.png`. Neither test modifies your desktop
or catalog. The live-provider test is opt-in, and compositor application is
verified with fake setters so tests do not change the desktop.

## Install and package

```sh
./scripts/cargo-safe build --release --workspace --locked
./scripts/build-gui
make install PREFIX=/usr/local
```

Use `DESTDIR=/temporary/staging` to stage an installation without writing system
files. Installation includes both clients, the daemon, a desktop entry, an icon,
a systemd user unit, and Bash/Zsh/Fish completions. Start manually or enable the user unit after installation:

```sh
systemctl --user enable --now wallfoliod.service
```

For Arch, create a source archive from your committed checkout, then use makepkg:

```sh
git archive --format=tar.gz --prefix=wallfolio-0.2.0/ HEAD > packaging/arch/wallfolio-0.2.0.tar.gz
cd packaging/arch
makepkg -s
```

With release binaries built, `python3 tests/install.py` verifies a temporary
installation using Zsh, desktop-file-validate, and systemd-analyze. It does not
install anything outside its temporary directory.

No service is enabled automatically. Wallpaper setters remain optional host tools.

## AppImage and GitHub releases

```sh
chmod +x Wallfolio-0.2.0-x86_64.AppImage
./Wallfolio-0.2.0-x86_64.AppImage           # GUI; starts daemon if needed
./Wallfolio-0.2.0-x86_64.AppImage cli search
./Wallfolio-0.2.0-x86_64.AppImage daemon    # Explicit daemon startup
```

The AppImage contains the GUI, CLI, daemon, and Qt dependencies. GUI startup
connects to an existing daemon or launches a separate invocation of the same
AppImage, keeping the daemon's bundle available after the GUI closes. The CLI
requires a running daemon. Data stays in the normal XDG directories outside the
bundle. Replacing the AppImage preserves it; restart the running daemon to use
the new version. Desktop wallpaper engines remain system dependencies.

Build on Linux x86_64 with the requirements above plus Python 3 and Qt SVG/Wayland
plugins:

```sh
./scripts/build-appimage
python3 tests/appimage.py dist/Wallfolio-0.2.0-x86_64.AppImage
```

Output is in `dist/` with a SHA-256 checksum. Build tools are downloaded with
pinned checksums from `packaging/appimage/tools.json`. If upstream continuous
artifacts change, update the manifest only after verifying the replacement. The
builder uses extraction instead of requiring FUSE. At runtime on a machine without
FUSE, set `APPIMAGE_EXTRACT_AND_RUN=1`.

GitHub Actions builds and tests on Ubuntu 24.04 for PRs, master pushes, and manual
runs, uploading an x86_64 AppImage artifact. Pushing a `v*` tag matching the Cargo
workspace version additionally publishes the verified AppImage and checksum to a
GitHub release. Ordinary PR/manual runs do not publish releases. Build on the CI
baseline for distribution: a bundle built on a newer local distribution can
require a newer glibc than Ubuntu 24.04.
