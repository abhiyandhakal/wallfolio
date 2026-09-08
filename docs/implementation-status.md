# Implementation status

The implementation covers the v0.1 and v0.2 milestones in architecture section 42.
The daemon arrived with v0.1 to preserve the process boundary; v0.2 adds its
scheduled work, cache, duplicate groups, Linux adapters, and portable packaging.

## v0.2 requirements and evidence

| Requirement | Implementation | Verification |
| --- | --- | --- |
| Daemon background work | Idle timer and a bounded single thumbnail worker | `tests/backends.py` waits without IPC and observes an apply |
| Random | Constant-memory local selection, favorites/exact tags, missing-file exclusion, repeat avoidance | Core tests, real CLI/API integration, GUI interaction test |
| Rotation | Opt-in persisted schedule, 30-minute default, missed-run coalescing, error status, start/stop | Core restart/tick tests, daemon idle/restart test, QML controls |
| Thumbnail cache | 256 MiB LRU eviction, persistent 512×320 previews, queue/decode/download bounds | Storage eviction/restart test, daemon generation/lookup test, GUI readiness test |
| Duplicates | SHA-256 groups, shared originals, separate catalog records, bounded group pagination | Core grouping test, shared-file lifecycle smoke test, GUI view |
| Linux backends | swww, Hyprpaper, swaybg, GNOME, KDE, Xfce, feh, xwallpaper, Nitrogen | Exact fake-command integration, monitor rejection, environment restoration; actual desktops are not modified |
| swaybg ownership | Replace only owned process; preserve old process on startup failure; parent-death cleanup | Integration verifies process replacement, failed start, switching away, daemon termination |
| AppImage | GUI/CLI/daemon launcher, bundled Qt, separate daemon mount lifetime, checksummed tools | Real AppImage smoke test; artifact must be rebuilt after source changes |
| GitHub releases | PR/master/manual build and artifact upload; matching version tags publish verified assets | Workflow source present; hosted CI verification pending |

## v0.1 requirements and evidence

| Requirement | Implementation | Verification |
| --- | --- | --- |
| Rust application core | `wallfolio-core` dispatches catalog, provider, storage, and apply services | Workspace build, tests, Clippy |
| SQLite catalog | Stable UUIDs; provenance uniqueness; metadata persisted separately from images; schema version guard | Catalog tests and daemon restart smoke test |
| Wallhaven provider | Public search without explicit filters, details, cataloguing, HTTPS image download | `tests/live_provider.py` uses the actual public service |
| Local folder provider | Paged single-directory discovery; PNG/JPEG/WebP imports | Isolated CLI smoke test |
| swww backend | Executable/session detection, optional monitor, timeout and exit-status errors | Fake executable captures exact arguments in smoke test |
| Hyprpaper backend | Current `hyprctl hyprpaper wallpaper` IPC, optional monitor, input validation | Fake executable captures exact arguments; official IPC reference linked in README |
| CLI | All catalog lifecycle operations, discovery, tags, favorites, device/backend information | Actual CLI processes talking to the daemon in smoke test |
| Qt/QML GUI | Library, Discover, Favorites, preview, import/save, download, tags, deletion, apply controls | CMake/qmake builds, offscreen runtime render, Qt Quick interaction tests |
| Search | Literal title/tag substring search, favorites filter, bounded pagination | Unit tests plus CLI and GUI interaction tests |
| Preview | Remote provider thumbnails; local full-image preview | Populated library render and preview/download interaction test |
| Save to catalog | Provider-independent IDs; re-adding the same provenance preserves ID | Local and live-provider tests |
| Download | Streamed copy; SHA-256 paths; complete decoding validation with memory limits | Storage tests, local and live-provider smoke tests |
| Delete local copy | Catalog state retained; shared files preserved; user sources untouched | Shared-content lifecycle assertions in smoke test |
| Remove catalog item | Metadata removed while managed file remains | CLI smoke test |
| Set wallpaper | Explicit or automatically selected backend receives managed file path | Both backend argument tests; a real compositor is intentionally not changed by tests |
| Favorites and tags | Core mutations, persistent search filters, GUI controls | Unit, CLI, restart, and GUI tests |

## Boundaries and delivery

- Qt contains presentation and IPC code, not catalog/provider/storage business logic.
- Core depends on provider/backend traits, never on the concrete adapter crate or Qt.
- The daemon constructs the adapters; neither client opens SQLite.
- One daemon owns each catalog, with serialized requests, bounded frames, timeouts,
  and stale-socket recovery. Invalid requests do not stop the daemon.
- Local operation needs no server. A provider failure produces an operation error;
  it does not restart the GUI's daemon or destroy local state.
- Builds use one job and memory limits. Image downloads and decoding are bounded.
- Linux delivery includes a staged Makefile install, Arch source-package recipe,
  desktop entry, scalable icon, shell completions, and optional systemd user unit.
  `tests/install.py` validates the staged files and user unit without enabling it.
- Feature-branch work and conventional commits follow AGENTS.md. No force push
  or changes to the user's wallpaper are required to build.

## Deliberate later work

As described in the architecture's later milestones: richer metadata/filtering;
triage keyboard workflow; collections/ratings; additional providers; custom
HTTP provider protocol; server/authentication/sync; Windows named pipes and native
Windows/macOS wallpaper application; other distribution formats.

The current catalog stores documents as SQLite JSON alongside indexed identity
and provenance columns. It does not yet implement the proposed normalized tables
for collections, devices, ratings, sync, and source registries. Provider and backend
adapters share one crate until their number warrants separate packages.

The preferred backend is persisted in the local catalog's device settings and
restored by the GUI; full multi-device profiles remain future work. Other
configuration is through CLI flags and XDG/WALLFOLIO_SOCKET environment variables; the
example architecture TOML file is not yet read. Wallhaven authentication, NSFW
queries, server sync, and arbitrary HTTP providers are not exposed as placeholder
commands. Search uses SQLite's ASCII case folding. Slow foreground network requests serialize
other client requests and rotation until they complete or time out. Thumbnail
generation runs independently on one worker. Automatic orphan cleanup is
not implemented, preserving the distinction between removal and local deletion.
