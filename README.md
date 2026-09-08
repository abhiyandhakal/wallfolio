# Wallfolio

A local-first wallpaper catalog with a Rust core, CLI, and Qt Quick desktop client.
Wallhaven and local directories provide discovery; catalog IDs belong to Wallfolio.
The optional `swww` and Hyprpaper adapters apply downloaded wallpapers on Wayland.

This implements the v0.1 scope in [ARCHITECTURE.md](ARCHITECTURE.md). A small socket
daemon is included now so the GUI and CLI share one catalog owner from the start.
Rotation, sync, other providers/backends, and Windows/macOS support remain later
milestones as specified in the architecture.

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

The GUI supports Library, Discover, Favorites, preview, import/save, download,
tags, deletion, and applying wallpapers. It displays thumbnails for remote
candidates and full local images after download. External wallpaper daemons must
already be running in the graphical session. Backend availability indicates that
the relevant executable and Wayland environment exist, not that its daemon is
healthy. The selected engine is saved immediately in the local catalog and
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

## Storage and protocol

Metadata lives in `$XDG_DATA_HOME/wallfolio/wallfolio.db` (by default
`~/.local/share/wallfolio/wallfolio.db`). Images live under `originals/<hash-prefix>/`.
`wallfoliod --data-dir PATH` selects an isolated catalog.

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
git archive --format=tar.gz --prefix=wallfolio-0.1.0/ HEAD > packaging/arch/wallfolio-0.1.0.tar.gz
cd packaging/arch
makepkg -s
```

With release binaries built, `python3 tests/install.py` verifies a temporary
installation using Zsh, desktop-file-validate, and systemd-analyze. It does not
install anything outside its temporary directory.

No service is enabled automatically. Wallpaper setters remain optional host tools.
