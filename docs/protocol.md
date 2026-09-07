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
| `provider.search` | `provider`, `query`, `page` (1-based) | Candidate array |
| `provider.get` | `provider`, `external_id` | Candidate |
| `wallpaper.download` | `id` | Wallpaper with local path/hash/dimensions |
| `wallpaper.delete_local` | `id` | Wallpaper, local path cleared |
| `wallpaper.apply` | `id`, optional `backend`, optional `monitor` | Applied status and backend |
| `device.info` | none | Version, operating system, desktop |
| `device.backends` | none | Backend availability and capabilities |

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
