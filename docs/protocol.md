# Wallfolio IPC v1

Transport: Unix domain socket, one UTF-8 JSON request per connection, terminated
by a newline. The daemon returns one newline-terminated JSON response and closes
the connection. Requests and responses are limited to 1 MiB including the newline.
Clients must handle errors and timeouts; a successful connection does not imply a
successful operation. There are no events or subscriptions in v1.

```json
{"version":1,"method":"catalog.search","params":{"query":"dark","limit":24,"offset":0}}
```

```json
{"ok":true,"result":[]}
```

```json
{"ok":false,"error":"unsupported protocol version"}
```

| Method | Parameters | Result |
| --- | --- | --- |
| `catalog.search` | `query` (default empty), `favorite` (false), `limit` (100, max 200), `offset` (0) | Wallpaper array, newest first |
| `catalog.get` | `id` | Wallpaper |
| `catalog.add` | `provider`, `external_id` | Existing or newly catalogued Wallpaper |
| `catalog.remove` | `id` | `{removed:true}`; files retained |
| `catalog.tags` | `id`, `tags` string array | Wallpaper with replacement tags |
| `favorite.add` / `favorite.remove` | `id` | Wallpaper |
| `provider.list` | none | Provider names |
| `provider.search` | `provider`, `query`, `page` (1-based) | Candidate array, enriched with Wallpaper fields for saved items |
| `provider.get` | `provider`, `external_id` | Candidate |
| `wallpaper.download` | `id` | Wallpaper with local path/hash/dimensions |
| `wallpaper.delete_local` | `id` | Wallpaper, local path cleared |
| `wallpaper.apply` | `id`, optional `backend`, optional `monitor` | Applied status and backend |
| `device.info` | none | Version, operating system, desktop, daemon PID |
| `device.backends` | none | Backend availability and capabilities |
| `device.settings` | none | `{preferred_backend: string or null}` from this local catalog |
| `device.settings.update` | `preferred_backend` (registered backend name) | Saved settings, without applying a wallpaper |

Wallpaper fields: `id`, `title`, `provider`, `external_id`, `source`, `thumbnail`,
`tags`, `favorite`, `local_path`, `content_hash`, `width`, and `height`. Optional
fields are JSON null. Candidate fields are the provenance/title/source/thumbnail/
tags subset without a catalog ID. Never treat an external provider ID as a catalog
ID. Local provider external IDs are absolute paths. The CLI resolves relative paths
against its own working directory before sending them. Other clients should also
send absolute paths to avoid dependence on the daemon working directory.

Tags are trimmed, empty tags removed, sorted, and deduplicated. At most 100 tags
of 100 bytes each are accepted. Search is literal substring matching over titles
and tags, using SQLite's ASCII case folding. Percent and underscore are literal,
not SQL wildcards. Oversized responses return an error; request a smaller limit.

Mutations are not blindly retried after an uncertain connection failure. In
particular, verify a catalog item before repeating an apply or delete operation.

Discovery enrichment matches provider and external ID, using one indexed lookup
for the returned page. Saved candidates carry their existing catalog `id`,
`favorite`, `local_path`, and other Wallpaper fields. Unsaved candidates have no
catalog ID. The GUI updates cached discovery cards after mutations, and new
searches read the latest persistent state.

Backend selection order for `wallpaper.apply` is explicit request, saved local
preference, then auto-detection. A successful explicit apply remembers that engine.
A missing or unavailable preferred backend produces an error; it does not silently
switch engines. GUI selections use `device.settings.update` immediately, so a
selection is retained even if the window closes before applying a wallpaper.

## v0.2 methods

| Method | Parameters | Result |
| --- | --- | --- |
| `wallpaper.random` | `favorite` (false), `tags` ([]), optional `backend`, `monitor` | `{applied, backend, wallpaper}` |
| `rotation.status` | none | Persisted rotation configuration/status |
| `rotation.configure` | `enabled` (false), `interval_seconds` (1800), `favorite` (false), `tags` ([]), optional `monitor` | Saved configuration with a new next-run time |
| `rotation.stop` | none | Disabled configuration; filters retained |
| `catalog.duplicates` | `limit` (20, max 50 groups), `offset` (0) | Groups `{content_hash, count, items}` with at most 20 entries each |
| `cache.status` | none | `{bytes, entries, max_bytes}` |
| `cache.lookup` | `keys` (up to 100 SHA-256 thumbnail keys) | Object mapping ready keys to local thumbnail paths |

Search and mutation results can include `thumbnail_key` and `cached_thumbnail`.
The latter is null until generation/download finishes. `cache.lookup` reads only
already-generated files; it never refetches a provider page. Cache files may be
evicted at any time, so clients should fall back to the source preview on a read
failure. Duplicate group items receive the same thumbnail enrichment.

Rotation status contains `enabled`, `interval_seconds`, `favorite`, `tags`,
`monitor`, `next_run`, `last_run`, and `last_error`. Times are Unix seconds or null.
The interval must be 10–604800 seconds. Reconfiguration resets the next run and
last status; clients cannot inject timestamps. Rotation uses the saved backend
preference and selects only existing downloaded library files. Exact tag filters
are combined with AND. Overdue rotation runs once when the daemon resumes.

Engine monitor capabilities are authoritative. GNOME, KDE, and feh reject a
nonempty monitor; Nitrogen requires a numeric head index. Xfce uses existing
background property names. swaybg processes are owned by the adapter and cleaned
up on replacement, switching away, and daemon exit on Linux.
