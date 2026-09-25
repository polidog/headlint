use std::collections::HashMap;
use std::io::Read;
use std::sync::LazyLock;
use std::time::Duration;

use scraper::{Html, Selector};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;
use url::Url;

use crate::robots::{allowed, parse_robots};

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    #[serde(rename = "ok")]
    Pass,
    Warn,
    Ng,
    Info,
}
use Level::*;

#[derive(Serialize)]
pub struct Item {
    pub label: String,
    pub value: String,
    #[serde(rename = "level")]
    pub lv: Level,
}

#[derive(Serialize)]
pub struct Section {
    pub name: &'static str,
    pub items: Vec<Item>,
}

impl Section {
    fn new(name: &'static str) -> Self {
        Section { name, items: Vec::new() }
    }
    fn add(&mut self, label: impl Into<String>, lv: Level, value: impl Into<String>) {
        self.items.push(Item { label: label.into(), value: value.into(), lv });
    }
}

struct Fetched {
    status: u16,
    url: Url, // リダイレクト後の URL
    content_type: String,
    x_robots: Vec<String>,
    body: Vec<u8>,
}

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; seo-checker/0.1)")
        .build()
});

fn get(u: &str) -> Result<Fetched, String> {
    let resp = match AGENT.get(u).call() {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(e.to_string()),
    };
    let url = Url::parse(resp.get_url()).map_err(|e| e.to_string())?;
    let status = resp.status();
    let content_type = resp.header("Content-Type").unwrap_or("").to_string();
    let x_robots = resp.all("X-Robots-Tag").into_iter().map(String::from).collect();
    let mut body = Vec::new();
    resp.into_reader().take(5 << 20).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(Fetched { status, url, content_type, x_robots, body })
}

/// URL を取得してステータス・Content-Type・サイズを返す。
fn probe(u: &str, want_type: &str) -> (Level, String) {
    let (lv, st, _) = probe_body(u, want_type);
    (lv, st)
}

fn probe_body(u: &str, want_type: &str) -> (Level, String, Vec<u8>) {
    let f = match get(u) {
        Ok(f) => f,
        Err(e) => return (Ng, format!("{u}  ({e})"), Vec::new()),
    };
    let s = format!("{u}  ({} {} {:.1}KB)", f.status, f.content_type, f.body.len() as f64 / 1024.0);
    let lv = if f.status != 200 {
        Ng
    } else if !f.content_type.starts_with(want_type) {
        Warn
    } else {
        Pass
    };
    (lv, s, f.body)
}

fn abs(base: &Url, r: &str) -> String {
    base.join(r.trim()).map_or(r.to_string(), String::from)
}

struct Link {
    rel: String,
    href: String,
    sizes: String,
}

struct Page {
    title: String,
    meta: HashMap<String, String>, // key: property / name (lowercase)
    links: Vec<Link>,
}

fn parse(body: &[u8]) -> Page {
    let doc = Html::parse_document(&String::from_utf8_lossy(body));
    let sel = |s| Selector::parse(s).unwrap();
    let title = doc
        .select(&sel("head > title"))
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let mut meta = HashMap::new();
    for e in doc.select(&sel("meta")) {
        let v = e.value();
        let k = v.attr("property").filter(|s| !s.is_empty()).or(v.attr("name")).unwrap_or("").to_lowercase();
        if !k.is_empty() {
            meta.entry(k).or_insert_with(|| v.attr("content").unwrap_or("").to_string());
        }
    }
    let links = doc
        .select(&sel("link"))
        .map(|e| {
            let a = |k| e.value().attr(k).unwrap_or("").to_string();
            Link { rel: a("rel").to_lowercase(), href: a("href"), sizes: a("sizes") }
        })
        .collect();
    Page { title, meta, links }
}

pub fn check(raw: &str) -> Result<Vec<Section>, String> {
    let f = get(raw)?;
    let p = parse(&f.body);
    let (rs, robots_txt) = check_robots(&f, &p);
    let mut raw_sec = Section::new("robots.txt");
    raw_sec.add("", Info, robots_txt);
    Ok(vec![check_ogp(&f.url, &p), check_favicon(&f.url, &p), rs, raw_sec])
}

fn counted(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    format!("{s}  [{}文字]", s.chars().count())
}

// 表示幅（全角=2）で数える。Google の検索結果で切れずに出るのは概ね幅60（全角30字）まで。
const TITLE_MIN_WIDTH: usize = 20;
const TITLE_MAX_WIDTH: usize = 60;

fn title_check(t: &str) -> (Level, String) {
    if t.is_empty() {
        return (Ng, String::new());
    }
    let w = t.width();
    let v = format!("{t}  [{}文字・幅{w}]", t.chars().count());
    let hint = format!("（目安: 幅{TITLE_MIN_WIDTH}〜{TITLE_MAX_WIDTH}＝全角{}〜{}字）", TITLE_MIN_WIDTH / 2, TITLE_MAX_WIDTH / 2);
    if w > TITLE_MAX_WIDTH {
        (Warn, format!("{v} 長すぎ・検索結果で省略される{hint}"))
    } else if w < TITLE_MIN_WIDTH {
        (Warn, format!("{v} 短すぎ{hint}"))
    } else {
        (Pass, v)
    }
}

/// OGP 画像サイズ判定。推奨 1200x630（1.91:1）、Facebook の最小は 200x200。
fn image_check(w: usize, h: usize) -> (Level, String) {
    let ratio = w as f64 / h as f64;
    let (mut lv, mut notes) = (Pass, Vec::new());
    if w < 200 || h < 200 {
        lv = Ng;
        notes.push("200x200 未満（表示されない）");
    } else if w < 1200 || h < 630 {
        lv = Warn;
        notes.push("推奨 1200x630 未満");
    }
    if (ratio - 1.91).abs() > 0.1 {
        if lv == Pass {
            lv = Warn;
        }
        notes.push("比率が 1.91:1 でない（切り抜かれる）");
    }
    (lv, format!("{w}x{h}（{ratio:.2}:1）  {}", notes.join(" / ")).trim_end().to_string())
}

fn og_image_size(s: &mut Section, p: &Page, body: &[u8]) {
    let size = match imagesize::blob_size(body) {
        Ok(size) => size,
        Err(e) => return s.add("og:image サイズ", Warn, format!("読み取れない（{e}）")),
    };
    let (mut lv, mut v) = image_check(size.width, size.height);
    if body.len() >= 5 << 20 {
        lv = if lv == Ng { Ng } else { Warn };
        v += " / 5MB 以上（X で表示されない）";
    }
    s.add("og:image サイズ", lv, v);
    // og:image:width/height の宣言値と実寸の食い違い
    for (k, actual) in [("og:image:width", size.width), ("og:image:height", size.height)] {
        if let Some(d) = p.meta.get(k).and_then(|v| v.trim().parse::<usize>().ok()) {
            if d != actual {
                s.add(format!("{k} 宣言値"), Warn, format!("{d} だが実寸は {actual}"));
            }
        }
    }
}

fn check_ogp(base: &Url, p: &Page) -> Section {
    let mut s = Section::new("OGP");
    let (lv, v) = title_check(&p.title);
    s.add("title", lv, v);
    match p.meta.get("description") {
        Some(d) => s.add("description", Pass, counted(d)),
        None => s.add("description", Warn, ""),
    }
    for (k, missing) in [
        ("og:title", Ng), ("og:type", Ng), ("og:url", Ng), ("og:image", Ng),
        ("og:description", Warn), ("og:site_name", Info), ("og:locale", Info),
        ("og:image:width", Info), ("og:image:height", Info), ("og:image:alt", Info),
        ("twitter:card", Warn), ("twitter:site", Info), ("twitter:title", Info),
        ("twitter:description", Info), ("twitter:image", Info),
    ] {
        match p.meta.get(k) {
            Some(v) => s.add(k, Pass, v),
            None => s.add(k, missing, ""),
        }
    }
    for k in ["og:image", "twitter:image"] {
        let Some(v) = p.meta.get(k).filter(|v| !v.is_empty()) else { continue };
        if !v.starts_with("http") {
            s.add(format!("{k} 形式"), Warn, "相対URL（絶対URLにすべき）");
        }
        let (lv, st, body) = probe_body(&abs(base, v), "image/");
        s.add(format!("{k} 取得"), lv, st);
        if k == "og:image" && lv != Ng {
            og_image_size(&mut s, p, &body);
        }
    }
    s
}

fn check_favicon(base: &Url, p: &Page) -> Section {
    let mut s = Section::new("Favicon");
    let mut found = false;
    for l in &p.links {
        if l.rel.contains("icon") {
            found = true;
            let label = if l.sizes.is_empty() { l.rel.clone() } else { format!("{} {}", l.rel, l.sizes) };
            let (lv, st) = probe(&abs(base, &l.href), "image/");
            s.add(label, lv, st);
        } else if l.rel == "manifest" {
            let (lv, st) = probe(&abs(base, &l.href), "");
            s.add("manifest", lv, st);
        }
    }
    if !found {
        s.add("link rel=icon", Warn, "");
    }
    let (lv, st) = probe(&abs(base, "/favicon.ico"), "image/");
    // link で指定済みなら無くても致命的ではない
    s.add("/favicon.ico", if lv == Ng { Warn } else { lv }, st);
    s
}

fn directive_level(v: &str) -> Level {
    let mut lv = Pass;
    for d in v.to_lowercase().split(',') {
        match d.trim() {
            "noindex" | "none" => return Ng,
            "nofollow" => lv = Warn,
            _ => {}
        }
    }
    lv
}

fn check_robots(f: &Fetched, p: &Page) -> (Section, String) {
    let base = &f.url;
    let mut s = Section::new("Robots");
    s.add("HTTP", if f.status == 200 { Pass } else { Ng }, format!("{}  {base}", f.status));

    match p.meta.get("robots") {
        Some(v) => s.add("meta robots", directive_level(v), v),
        None => s.add("meta robots", Pass, "未指定（index, follow）"),
    }
    if let Some(v) = p.meta.get("googlebot") {
        s.add("meta googlebot", directive_level(v), v);
    }
    let xr = f.x_robots.join(", ");
    if xr.is_empty() {
        s.add("X-Robots-Tag", Pass, "未指定");
    } else {
        s.add("X-Robots-Tag", directive_level(&xr), xr);
    }

    // Url は "https://a.com" を "https://a.com/" に正規化するので文字列比較で足りる
    match p.links.iter().find(|l| l.rel == "canonical").map(|l| abs(base, &l.href)) {
        None => s.add("canonical", Warn, ""),
        Some(c) if c != base.as_str() => s.add("canonical", Warn, format!("{c}  (このページと別URL)")),
        Some(c) => s.add("canonical", Pass, c),
    }

    let robots_url = abs(base, "/robots.txt");
    let r = match get(&robots_url) {
        Ok(r) => r,
        Err(e) => {
            s.add("robots.txt", Ng, &e);
            return (s, e);
        }
    };
    if r.status >= 500 {
        s.add("robots.txt", Ng, format!("{}（Google は全ブロック扱い）", r.status));
        return (s, r.status.to_string());
    } else if r.status != 200 {
        s.add("robots.txt", Warn, format!("{}（無し＝全クロール許可扱い）", r.status));
        return (s, r.status.to_string());
    } else if !r.content_type.starts_with("text/plain") {
        s.add("robots.txt", Warn, format!("200 だが Content-Type が {}", r.content_type));
    } else {
        s.add("robots.txt", Pass, format!("200 {robots_url}"));
    }

    let txt = String::from_utf8_lossy(&r.body).into_owned();
    let (groups, sitemaps) = parse_robots(&txt);
    let ua = if groups.contains_key("googlebot") { "googlebot" } else { "*" };
    let rules = groups.get(ua).map_or(&[][..], |v| v);
    let path = match base.query() {
        Some(q) => format!("{}?{q}", base.path()),
        None => base.path().to_string(),
    };
    match allowed(rules, &path) {
        (true, by) => s.add("クロール可否", Pass, format!("許可（User-agent: {ua}）{by}")),
        (false, by) => s.add("クロール可否", Ng, format!("ブロック（User-agent: {ua}）{by}")),
    }

    if sitemaps.is_empty() {
        s.add("Sitemap", Warn, "robots.txt に記載なし");
    }
    for sm in &sitemaps {
        let (lv, st) = probe(sm, "");
        s.add("Sitemap", lv, st);
    }
    (s, txt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title() {
        assert!(title_check("") == (Ng, String::new()));
        assert!(title_check("短い").0 == Warn);
        assert!(title_check("ちょうどいい長さのタイトルです｜サイト名").0 == Pass); // 幅40
        assert!(title_check(&"あ".repeat(31)).0 == Warn); // 幅62
    }

    #[test]
    fn image() {
        assert!(image_check(1200, 630).0 == Pass);
        assert!(image_check(2400, 1260).0 == Pass);
        assert!(image_check(800, 419).0 == Warn); // 比率OK・小さい
        assert!(image_check(1200, 1200).0 == Warn); // 正方形
        assert!(image_check(150, 150).0 == Ng);
    }
}
