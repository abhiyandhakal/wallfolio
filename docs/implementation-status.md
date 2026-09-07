# Implementation status

The implementation follows the v0.1 milestone in architecture section 42, which
explicitly says not to build every future component immediately. The small daemon
arrives with v0.1 to preserve the process boundaries in sections 4 and 18; scheduled
background jobs remain v0.2 work.

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
- Feature-branch work and conventional commits follow AGENTS.md. No force push,
  remote publication, or changes to the user's wallpaper are required to build.

## Deliberate later work

As described in the architecture's later milestones: rotation and background job
queues; generated thumbnail cache; richer metadata/filtering; triage keyboard
workflow; collections/ratings; additional providers and desktop backends; custom
HTTP provider protocol; server/authentication/sync; Windows named pipes and native
Windows/macOS wallpaper application; other distribution formats.

The current catalog stores documents as SQLite JSON alongside indexed identity
and provenance columns. It does not yet implement the proposed normalized tables
for collections, devices, ratings, sync, and source registries. Provider and backend
adapters share one crate until their number warrants separate packages.

Backend choice is per request, not a persisted device profile. Configuration is
currently through CLI flags and XDG/WALLFOLIO_SOCKET environment variables; the
example architecture TOML file is not yet read. Wallhaven authentication, NSFW
queries, server sync, and arbitrary HTTP providers are not exposed as placeholder
commands. Search uses SQLite's ASCII case folding. Slow network requests serialize
other daemon work until they complete or time out. Automatic orphan cleanup is
not implemented, preserving the distinction between removal and local deletion.
