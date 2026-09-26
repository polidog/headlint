# headlint

A TUI / CLI tool that lints a page's `<head>` (OGP, title, favicon, canonical, meta robots) and its robots.txt.

```
cargo install --git https://github.com/polidog/headlint
```

## Usage

```
headlint polidog.jp                     # show results in the TUI
headlint --json polidog.jp              # print results as JSON
headlint --validate polidog.jp          # print results as text, exit 1 if any ✗
headlint --json --validate polidog.jp   # print JSON, exit 1 if any ✗
```

TUI keys: `←/→` `h/l` `Tab` to switch tabs, `↑/↓` `j/k` `PgUp/PgDn` to scroll, `q` to quit.

Check messages are currently in Japanese.

## Checks

Each result is ✓ ok / ! warning / ✗ error / - info.

- **OGP**: title (length), description, og:\*, twitter:\*, and og:image fetch and size (recommended 1200x630 at 1.91:1, minimum 200x200, mismatch with declared og:image:width/height)
- **Favicon**: fetches every `link` whose `rel` contains `icon`, the manifest, and `/favicon.ico`
- **Robots**: HTTP status, meta robots / googlebot, X-Robots-Tag, canonical, and robots.txt (status, Content-Type, whether this URL is crawlable, Sitemap fetch)
- **robots.txt**: the raw file

## JSON

```json
{ "url": "...", "ok": true,
  "sections": [ { "name": "OGP", "items": [ { "label": "og:title", "level": "ok", "value": "..." } ] } ],
  "robots_txt": "User-agent: *\n..." }
```

`level` is one of `ok` / `warn` / `ng` / `info`. `ok` is `true` when there is no `ng`.

## License

MIT
