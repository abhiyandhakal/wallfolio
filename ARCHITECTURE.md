# Wallfolio — Overall Architecture

> Personal, cross-platform wallpaper catalog with GUI + CLI, provider-independent discovery, local-first storage, optional self-hosted sync, and platform-specific wallpaper backends.

---

## 1. Core Design Principles

Wallfolio should be built around five independent concepts:

1. **Catalog**

   * Owns wallpaper identity.
   * Independent of Wallhaven, Unsplash, local files, or any other source.

2. **Providers**

   * Discover wallpapers.
   * Providers are sources, not the database.

3. **Clients**

   * GUI, CLI, daemon, and potentially web/mobile clients.
   * All clients operate through the same application core.

4. **Wallpaper Backends**

   * Apply wallpapers using platform-specific mechanisms.
   * Hyprland, GNOME, KDE, Xfce, i3, etc. are implementation details.

5. **Sync**

   * Optional.
   * Local Wallfolio must remain fully usable without a server.

---

# 2. High-Level Architecture

```text
                           ┌──────────────────────────────┐
                           │       WALLFOLIO SERVER       │
                           │          Optional            │
                           │                              │
                           │  Catalog API                 │
                           │  Authentication              │
                           │  Object/File Storage         │
                           │  Device Sync                 │
                           │  Collections                 │
                           └──────────────┬───────────────┘
                                          │
                                          │ Sync
                                          │
                                          ▼
┌──────────────────────────────────────────────────────────────────────┐
│                         WALLFOLIO CLIENT                              │
│                                                                      │
│  ┌─────────────┐   ┌─────────────┐   ┌───────────────┐              │
│  │ Qt/QML GUI  │   │ Rust CLI    │   │ wallfoliod    │              │
│  │             │   │             │   │ background    │              │
│  └──────┬──────┘   └──────┬──────┘   └───────┬───────┘              │
│         │                 │                  │                       │
│         └─────────────────┼──────────────────┘                       │
│                           │                                          │
│                           ▼                                          │
│                  ┌───────────────────┐                               │
│                  │ Application Core  │                               │
│                  │       Rust        │                               │
│                  └─────────┬─────────┘                               │
│                            │                                         │
│       ┌────────────────────┼──────────────────────┐                  │
│       │                    │                      │                  │
│       ▼                    ▼                      ▼                  │
│  Providers            Local Catalog        Wallpaper Backends       │
│                                                                      │
│ Wallhaven             SQLite                Hyprpaper                │
│ Unsplash              Files                 swww                     │
│ Local Folder          Thumbnails            swaybg                   │
│ HTTP API              Cache                 GNOME                    │
│ Personal API          Metadata              KDE                      │
│ Manual URL                                  Xfce                     │
│                                             feh                      │
│                                             xwallpaper               │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

# 3. Technology Stack

## Core

```text
Language: Rust
```

Rust handles:

* application logic
* catalog management
* provider system
* downloads
* metadata extraction
* cache
* sync
* backend detection
* wallpaper application
* CLI
* daemon

---

## GUI

```text
Qt 6
Qt Quick
QML
```

Qt/QML is only the presentation layer.

The Qt GUI must **not contain Wallfolio business logic**.

```text
Qt/QML
   │
   ▼
Wallfolio protocol/API
   │
   ▼
Rust core
```

This allows the GUI to be replaced later without rewriting Wallfolio.

---

## CLI

```text
Rust
```

Example:

```bash
wallfolio search "dark mountains"

wallfolio provider search wallhaven "dark landscape"

wallfolio add <id>

wallfolio download <id>

wallfolio set <id>

wallfolio random

wallfolio favorite <id>

wallfolio remove-local <id>

wallfolio remove <id>

wallfolio sync
```

---

## Background Service

```text
wallfoliod
```

Responsibilities:

* wallpaper rotation
* background downloads
* scheduled sync
* thumbnail generation
* metadata extraction
* cache cleanup
* source refresh
* folder watching
* duplicate detection

---

# 4. Process Architecture

Preferred runtime architecture:

```text
┌──────────────────┐
│ wallfolio-gui    │
│ Qt / QML         │
└────────┬─────────┘
         │
         │ IPC
         ▼
┌──────────────────┐
│ wallfoliod       │
│ Rust daemon      │
│                  │
│ Application Core │
│ SQLite           │
│ Providers        │
│ Backends         │
└────────▲─────────┘
         │
         │ IPC
┌────────┴─────────┐
│ wallfolio CLI    │
│ Rust             │
└──────────────────┘
```

The GUI and CLI are peers.

Neither owns the application state.

---

# 5. IPC Layer

## Linux / macOS

Use:

```text
Unix Domain Socket
```

Example:

```text
$XDG_RUNTIME_DIR/wallfolio/wallfoliod.sock
```

---

## Windows

Future implementation:

```text
Named Pipes
```

---

## Protocol

Use a simple versioned JSON protocol initially.

Example:

```json
{
  "version": 1,
  "method": "catalog.search",
  "params": {
    "tags": ["dark", "landscape"]
  }
}
```

Response:

```json
{
  "ok": true,
  "result": [
    {
      "id": "019...",
      "title": "Dark Mountain Valley"
    }
  ]
}
```

Possible methods:

```text
catalog.search
catalog.get
catalog.add
catalog.remove

provider.search
provider.get

wallpaper.download
wallpaper.delete_local
wallpaper.apply

collection.create
collection.add

favorite.add
favorite.remove

device.info
device.backends

sync.start
sync.status
```

---

# 6. Application Core

```text
wallfolio-core
```

Contains:

```text
Application Services
├── CatalogService
├── SearchService
├── ProviderService
├── DownloadService
├── CacheService
├── ThumbnailService
├── MetadataService
├── CollectionService
├── FavoriteService
├── DeviceService
├── SyncService
└── WallpaperApplyService
```

---

# 7. Wallpaper Lifecycle

A wallpaper should have separate catalog and local-file states.

```text
DISCOVERED
    │
    ▼
CATALOGUED
    │
    ▼
CACHED LOCALLY
    │
    ▼
APPLIED
```

---

## Discovered

Known only through a provider.

```text
Remote metadata
Thumbnail
Provider ID
```

Full image is not necessarily downloaded.

---

## Catalogued

User decided the wallpaper belongs in their collection.

Wallfolio creates its own identity.

Example:

```json
{
  "id": "019af...",
  "title": "Foggy Mountains",
  "tags": [
    "dark",
    "landscape",
    "mountain",
    "fog"
  ]
}
```

Provider identity is only provenance.

---

## Cached Locally

Full-resolution wallpaper exists on this device.

```text
catalogued = true
local_file = true
```

---

## Delete Local Copy

Equivalent to the earlier concept of "uninstall".

```text
Catalog entry remains.

Full-resolution file is deleted.
```

---

## Remove From Catalog

Deletes the wallpaper from the user's collection.

This is separate from deleting the local file.

---

# 8. Provider Architecture

```rust
trait WallpaperProvider {
    fn id(&self) -> ProviderId;

    async fn search(
        &self,
        query: SearchQuery
    ) -> Result<Vec<WallpaperCandidate>>;

    async fn get(
        &self,
        external_id: &str
    ) -> Result<WallpaperCandidate>;

    async fn download(
        &self,
        external_id: &str
    ) -> Result<DownloadSource>;
}
```

---

## Built-In Providers

```text
WallhavenProvider
UnsplashProvider
LocalFolderProvider
ManualURLProvider
WallfolioServerProvider
GenericHTTPProvider
```

Later:

```text
PexelsProvider
GitHubRepoProvider
WallpaperEngineProvider
etc.
```

---

# 9. Custom Provider API

Wallfolio should eventually define a standard provider protocol.

Example server:

```text
https://wallpapers.example.com/
```

Manifest:

```text
GET /.well-known/wallfolio-provider.json
```

Example:

```json
{
  "name": "My Wallpaper Catalog",
  "api_version": 1,
  "base_url": "https://wallpapers.example.com/api"
}
```

Endpoints:

```text
GET /wallpapers
GET /wallpapers/{id}
GET /wallpapers/{id}/download
```

This allows users to add arbitrary compatible catalogs without loading arbitrary code inside Wallfolio.

---

# 10. Local Storage

## Database

Use:

```text
SQLite
```

Location:

```text
~/.local/share/wallfolio/wallfolio.db
```

SQLite stores metadata only.

Do not store wallpaper image blobs inside SQLite.

---

## Tables

Possible schema:

```text
wallpapers
sources
wallpaper_sources
tags
wallpaper_tags
collections
collection_items
favorites
ratings
downloads
devices
device_settings
apply_history
provider_configs
sync_state
```

---

# 11. Filesystem Layout

```text
~/.local/share/wallfolio/
├── wallfolio.db
├── originals/
│   ├── ab/
│   │   └── abcdef...
│   └── ...
├── thumbnails/
├── previews/
└── imports/
```

Cache:

```text
~/.cache/wallfolio/
├── provider-thumbnails/
├── temporary-downloads/
└── generated/
```

Configuration:

```text
~/.config/wallfolio/
└── config.toml
```

---

# 12. Content Addressing

Wallpaper files should ideally be keyed by content hash.

Example:

```text
SHA-256(image)
```

Benefits:

* duplicate detection
* provider-independent identity
* safe re-import
* easy synchronization
* deduplication

Example:

```text
wallhaven image
        │
        ▼
SHA256: abcdef...
        │
        ▼
already exists?
   │          │
 yes         no
   │          │
 reuse      add
```

---

# 13. Wallpaper Backend Architecture

Providers and wallpaper setters must be completely separate.

```rust
trait WallpaperBackend {
    fn id(&self) -> BackendId;

    async fn detect(
        &self
    ) -> BackendDetection;

    async fn monitors(
        &self
    ) -> Result<Vec<Monitor>>;

    async fn apply(
        &self,
        request: ApplyRequest
    ) -> Result<()>;

    fn capabilities(
        &self
    ) -> BackendCapabilities;
}
```

---

## Linux Backends

### Wayland

```text
HyprpaperBackend
SwwwBackend
SwaybgBackend
GnomeBackend
KdeBackend
```

### X11

```text
FehBackend
XwallpaperBackend
NitrogenBackend
XfceBackend
```

---

## Future

```text
WindowsBackend
MacOSBackend
```

---

# 14. Backend Detection

Detect:

```text
XDG_CURRENT_DESKTOP
XDG_SESSION_DESKTOP
XDG_SESSION_TYPE
WAYLAND_DISPLAY
DISPLAY
```

Also check executables:

```text
hyprpaper
swww
swaybg
feh
xwallpaper
nitrogen
```

Example:

```text
Desktop:
Hyprland

Session:
Wayland

Available:
✓ swww
✓ hyprpaper
✓ swaybg
```

UI:

```text
Wallpaper backend

● swww
○ hyprpaper
○ swaybg
```

User selection overrides auto-detection.

---

# 15. Backend Capabilities

Backends expose capabilities.

```rust
struct BackendCapabilities {
    per_monitor: bool,
    transitions: bool,
    animated: bool,
    fit_modes: bool,
}
```

Example:

```text
swww

Per-monitor    ✓
Transitions    ✓
Animated       ✓
```

GNOME:

```text
Per-monitor    limited
Transitions    ✗
Animated       ✗
```

The GUI adapts accordingly.

---

# 16. Device Profiles

Backend configuration must be per-device.

Example:

```json
{
  "device_id": "desktop-main",
  "hostname": "archbox",
  "backend": "swww",
  "default_monitor": "DP-1"
}
```

Synced catalog:

```text
✓
```

Synced backend configuration:

```text
No, unless stored under that specific device.
```

---

# 17. Qt GUI Architecture

```text
Qt Quick
QML
```

Views:

```text
Discover
Library
Favorites
Collections
Downloads
Devices
Sources
Settings
```

---

## Example GUI Structure

```text
ApplicationWindow
│
├── Sidebar
│   ├── Discover
│   ├── Library
│   ├── Favorites
│   ├── Collections
│   ├── Downloads
│   ├── Sources
│   └── Settings
│
└── ContentStack
    ├── DiscoverView
    ├── LibraryView
    ├── WallpaperView
    ├── CollectionView
    └── SettingsView
```

Reusable QML components:

```text
WallpaperCard
WallpaperGrid
WallpaperPreview
ProviderSelector
FilterPanel
TagChip
DownloadButton
FavoriteButton
BackendSelector
MonitorSelector
```

---

# 18. Qt ↔ Rust Boundary

Avoid deeply binding Rust application objects directly into Qt.

Preferred:

```text
QML
 │
 ▼
Thin Qt/C++ models
 │
 ▼
Wallfolio IPC client
 │
 ▼
wallfoliod
 │
 ▼
Rust core
```

Qt-side responsibilities:

```text
rendering
models
view state
IPC requests
IPC event handling
```

Rust responsibilities:

```text
everything else
```

---

# 19. Discover Flow

```text
User opens Discover
        │
        ▼
Select Provider
        │
        ▼
Wallhaven / Unsplash / Personal Catalog
        │
        ▼
Search
        │
        ▼
Candidates
        │
        ▼
Thumbnail Grid
        │
   ┌────┴─────┐
   │          │
 Skip       Add
              │
              ▼
        Local Catalog
```

---

# 20. Triage Mode

Important feature.

```text
┌───────────────────────────────┐
│                               │
│       LARGE WALLPAPER         │
│                               │
│                               │
├───────────────────────────────┤
│        Skip       Save        │
└───────────────────────────────┘
```

Keyboard:

```text
← / h      Skip
→ / l      Save
d          Download
f          Favorite
Enter      Full preview
Esc        Back
```

---

# 21. Local Library Flow

```text
Library
  │
  ├── All
  ├── Favorites
  ├── Downloaded
  ├── Remote-only
  ├── Dark
  ├── Landscape
  ├── Anime
  └── Collections
```

Filters:

```text
tags
rating
provider
resolution
aspect ratio
dominant color
brightness
download state
favorite state
```

---

# 22. Metadata

Automatically extract:

```text
width
height
aspect ratio
format
file size
average luminance
dominant colors
content hash
EXIF where available
```

Potential future metadata:

```text
semantic embedding
AI-generated tags
similarity score
aesthetic score
```

---

# 23. Sync Architecture

Wallfolio should be local-first.

```text
Local Catalog
     │
     ▼
Sync Engine
     │
     ▼
Wallfolio Server
```

Server unavailable:

```text
Local client continues working.
```

When server returns:

```text
sync resumes.
```

---

# 24. Self-Hosted Server

Future component:

```text
wallfolio-server
```

Responsibilities:

```text
authentication
catalog sync
wallpaper metadata
collections
device state
object storage
backups
```

Possible API:

```text
/api/v1/wallpapers
/api/v1/collections
/api/v1/devices
/api/v1/sync
/api/v1/files
```

---

# 25. Server Storage

Metadata:

```text
PostgreSQL
```

Files:

```text
Filesystem
or
S3-compatible object storage
```

Examples:

```text
MinIO
Cloudflare R2
AWS S3
Backblaze B2
```

Do not couple Wallfolio to one object-storage provider.

---

# 26. Authentication

For personal use initially:

```text
API token
```

Later:

```text
OAuth
OIDC
Passkeys
```

Do not implement complex authentication in v1.

---

# 27. Packaging Strategy

Packaging must influence architecture from the start.

Wallfolio consists of:

```text
wallfolio
wallfolio-gui
wallfoliod
```

Plus:

```text
desktop entry
icons
system service
shell completions
configuration
```

---

# 28. Linux Packaging

## Native Package

Preferred primary distribution.

Package contains:

```text
/usr/bin/wallfolio
/usr/bin/wallfolio-gui
/usr/bin/wallfoliod
```

Desktop integration:

```text
/usr/share/applications/io.wallfolio.Wallfolio.desktop
```

Icons:

```text
/usr/share/icons/hicolor/
```

Systemd user service:

```text
/usr/lib/systemd/user/wallfoliod.service
```

---

# 29. Arch Linux

Initial supported package:

```text
PKGBUILD
```

Potentially:

```text
AUR
```

Native package should depend on system Qt.

Example dependencies:

```text
qt6-base
qt6-declarative
qt6-imageformats
```

Wallfolio's external wallpaper setters remain optional dependencies.

Example:

```text
optdepends:
  swww
  hyprpaper
  swaybg
  feh
  xwallpaper
  nitrogen
```

---

# 30. Debian / Ubuntu

Later:

```text
.deb
```

Potential split:

```text
wallfolio
wallfolio-gui
```

But initially one package is simpler.

---

# 31. Fedora

Later:

```text
.rpm
```

---

# 32. AppImage

Useful as portable Linux distribution.

Bundle:

```text
Qt libraries
wallfolio-gui
wallfolio
wallfoliod
```

Do not bundle:

```text
swww
hyprpaper
feh
etc.
```

Those are host capabilities.

---

# 33. Flatpak

Do **not** make Flatpak the primary distribution initially.

Reason:

Wallfolio needs to:

```text
detect session
detect compositor
run wallpaper tools
inspect monitors
communicate with compositor services
access user wallpaper files
```

Flatpak sandboxing complicates exactly these operations.

Flatpak can be added later if:

```text
Portals
or
host-side Wallfolio daemon
```

provide a clean integration path.

---

# 34. Windows Packaging

Future.

Use:

```text
Qt deployment tooling
```

Package:

```text
wallfolio.exe
wallfolio-gui.exe
wallfoliod.exe
Qt runtime libraries
```

Installer:

```text
MSIX
or
WiX
or
NSIS
```

Backend:

```text
WindowsWallpaperBackend
```

---

# 35. macOS Packaging

Future.

```text
Wallfolio.app
```

Bundle:

```text
Qt frameworks
GUI
Rust core/daemon
```

Package as:

```text
DMG
```

Backend:

```text
MacOSWallpaperBackend
```

---

# 36. Repository Structure

```text
wallfolio/
│
├── Cargo.toml
├── README.md
├── LICENSE
│
├── crates/
│   │
│   ├── wallfolio-core/
│   │
│   ├── wallfolio-catalog/
│   │
│   ├── wallfolio-protocol/
│   │
│   ├── wallfolio-provider-api/
│   │
│   ├── wallfolio-backend-api/
│   │
│   ├── wallfolio-storage/
│   │
│   ├── wallfolio-sync/
│   │
│   ├── wallfolio-cli/
│   │
│   └── wallfoliod/
│   │
│   ├── providers/
│   │   ├── wallhaven/
│   │   ├── unsplash/
│   │   ├── local/
│   │   ├── http/
│   │   └── wallfolio-server/
│   │
│   └── backends/
│       ├── hyprpaper/
│       ├── swww/
│       ├── swaybg/
│       ├── gnome/
│       ├── kde/
│       ├── xfce/
│       ├── feh/
│       └── xwallpaper/
│
├── gui/
│   ├── CMakeLists.txt
│   ├── src/
│   │   ├── main.cpp
│   │   ├── ipc/
│   │   └── models/
│   │
│   └── qml/
│       ├── Main.qml
│       ├── views/
│       ├── components/
│       └── dialogs/
│
├── packaging/
│   ├── arch/
│   │   └── PKGBUILD
│   ├── debian/
│   ├── rpm/
│   ├── appimage/
│   ├── windows/
│   ├── macos/
│   ├── systemd/
│   │   └── wallfoliod.service
│   └── desktop/
│       └── io.wallfolio.Wallfolio.desktop
│
├── schemas/
│   ├── protocol/
│   └── provider/
│
├── assets/
│   ├── icons/
│   └── branding/
│
└── server/
    └── future/
```

---

# 37. Crate Dependency Direction

```text
wallfolio-core
      ▲
      │
 ┌────┴─────┐
 │          │
catalog   sync
 │          │
 └────┬─────┘
      │
providers/backends
```

Important rule:

```text
wallfolio-core
```

must never depend on:

```text
Qt
QML
Hyprland
GNOME
Wallhaven
Unsplash
```

Those are adapters.

---

# 38. Configuration

Example:

```toml
[general]
download_dir = "~/.local/share/wallfolio/originals"

[daemon]
enabled = true

[backend]
preferred = "swww"

[cache]
max_size_gb = 5

[sync]
enabled = false
```

Provider configuration:

```toml
[providers.wallhaven]
enabled = true

[providers.unsplash]
enabled = false

[providers.personal]
enabled = true
url = "https://wallpapers.example.com"
```

---

# 39. Systemd User Service

Example conceptual unit:

```ini
[Unit]
Description=Wallfolio background daemon

[Service]
ExecStart=/usr/bin/wallfoliod
Restart=on-failure

[Install]
WantedBy=default.target
```

The GUI should also be capable of launching the daemon if it is unavailable.

---

# 40. Startup Behavior

```text
wallfolio-gui starts
        │
        ▼
Check daemon socket
        │
   ┌────┴────┐
   │         │
exists     absent
   │         │
connect    spawn wallfoliod
             │
             ▼
           connect
```

---

# 41. Failure Isolation

Provider failure:

```text
Wallhaven unavailable

→ local library still works
```

Wallpaper backend failure:

```text
swww unavailable

→ discovery/catalog still works
```

Server failure:

```text
Wallfolio server unavailable

→ local operation continues
```

GUI failure:

```text
CLI + daemon remain functional
```

Daemon failure:

```text
GUI detects it and restarts/reconnects
```

---

# 42. MVP Scope

Do not build everything immediately.

## v0.1

```text
Rust core
SQLite catalog
Wallhaven provider
Local folder provider
swww backend
Hyprpaper backend
CLI
Qt/QML GUI
```

Features:

```text
search
preview
save to catalog
download
delete local copy
remove catalog item
set wallpaper
favorites
tags
```

---

## v0.2

```text
daemon
rotation
thumbnail cache
duplicate detection
more Linux backends
```

---

## v0.3

```text
custom HTTP provider
personal catalog server
sync
```

---

## v1.0

Potential target:

```text
stable provider API
stable backend API
stable IPC protocol
Linux packaging
multi-device sync
robust GUI
CLI parity
```

---

# 43. Final Architecture Summary

```text
                         Remote Sources
                  ┌─────────┬───────────┐
                  │         │           │
              Wallhaven  Unsplash   Custom APIs
                  │         │           │
                  └──── Provider Layer ─┘
                            │
                            ▼
                       Rust Core
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
   SQLite Catalog       File Storage       Apply Layer
        │                                       │
        │                           ┌───────────┼──────────┐
        │                           │           │          │
        │                         swww      Hyprpaper    GNOME
        │
        ▼
   Optional Sync
        │
        ▼
 Wallfolio Server

        ▲
        │
 ┌──────┼───────┐
 │      │       │
GUI    CLI    Daemon
Qt     Rust    Rust
QML
```

---

# 44. Architectural Rule of Thumb

Whenever adding a new feature, ask:

```text
Is this...

a source?
→ Provider

part of the user's collection?
→ Catalog

something this machine does?
→ Device/backend layer

something the user interacts through?
→ Client

something shared between devices?
→ Sync/server
```

If those boundaries remain intact, Wallfolio can grow from:

```text
"my Hyprland wallpaper browser"
```

into:

```text
"a platform-independent personal wallpaper catalog"
```

without requiring a rewrite.

