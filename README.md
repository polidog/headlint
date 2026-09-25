# seo-checker

URL を渡すと OGP・ファビコン・robots まわりをチェックする TUI / CLI ツール。

```
cargo install --git https://github.com/polidog/seo-checker
```

## 使い方

```
seo-checker polidog.jp                     # TUI で表示
seo-checker --json polidog.jp              # JSON で出力
seo-checker --validate polidog.jp          # テキストで出力し、✗ があれば exit 1
seo-checker --json --validate polidog.jp   # JSON で出力し、✗ があれば exit 1
```

TUI のキー: `←/→` `h/l` `Tab` でタブ切替、`↑/↓` `j/k` `PgUp/PgDn` でスクロール、`q` で終了。

## チェック内容

判定は ✓ 問題なし / ! 注意 / ✗ 問題あり / - 参考情報。

- **OGP**: title（長さ）、description、og:\*、twitter:\*、og:image の取得とサイズ（推奨 1200x630・1.91:1、最小 200x200、宣言値との食い違い）
- **Favicon**: `link rel` に icon を含むもの・manifest の取得、`/favicon.ico`
- **Robots**: HTTP ステータス、meta robots / googlebot、X-Robots-Tag、canonical、robots.txt（ステータス・Content-Type・このURLのクロール可否・Sitemap の取得）
- **robots.txt**: 本文

## JSON

```json
{ "url": "...", "ok": true,
  "sections": [ { "name": "OGP", "items": [ { "label": "og:title", "level": "ok", "value": "..." } ] } ] }
```

`level` は `ok` / `warn` / `ng` / `info`。`ok` は `ng` が 1 つもないとき `true`。

## License

MIT
