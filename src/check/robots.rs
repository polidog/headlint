use super::resolve;
use crate::fetch::{self, Response};
use crate::page::Page;
use crate::report::Level::{self, *};
use crate::report::Section;
use crate::robots_txt::{Kind, RobotsTxt, decisive_rule};

/// Robots セクションと、取得できた robots.txt 本文を返す。
pub fn check(res: &Response, p: &Page) -> (Section, Option<String>) {
    let base = &res.url;
    let mut s = Section::new("Robots");
    s.add(
        "HTTP",
        if res.status == 200 { Pass } else { Ng },
        format!("{}  {base}", res.status),
    );

    match p.meta("robots") {
        Some(v) => s.add("meta robots", directive_level(v), v),
        None => s.add("meta robots", Pass, "未指定（index, follow）"),
    }
    if let Some(v) = p.meta("googlebot") {
        s.add("meta googlebot", directive_level(v), v);
    }
    let xr = res.x_robots.join(", ");
    if xr.is_empty() {
        s.add("X-Robots-Tag", Pass, "未指定");
    } else {
        s.add("X-Robots-Tag", directive_level(&xr), xr);
    }

    // Url は "https://a.com" を "https://a.com/" に正規化するので文字列比較で足りる
    match p.link("canonical").map(|l| resolve(base, &l.href)) {
        None => s.add("canonical", Warn, ""),
        Some(c) if c != base.as_str() => {
            s.add("canonical", Warn, format!("{c}  (このページと別URL)"))
        }
        Some(c) => s.add("canonical", Pass, c),
    }

    let robots_url = resolve(base, "/robots.txt");
    let r = match fetch::get(&robots_url) {
        Ok(r) => r,
        Err(e) => {
            s.add("robots.txt", Ng, e.to_string());
            return (s, None);
        }
    };
    if r.status >= 500 {
        s.add(
            "robots.txt",
            Ng,
            format!("{}（Google は全ブロック扱い）", r.status),
        );
        return (s, None);
    } else if r.status != 200 {
        s.add(
            "robots.txt",
            Warn,
            format!("{}（無し＝全クロール許可扱い）", r.status),
        );
        return (s, None);
    } else if !r.content_type.starts_with("text/plain") {
        s.add(
            "robots.txt",
            Warn,
            format!("200 だが Content-Type が {}", r.content_type),
        );
    } else {
        s.add("robots.txt", Pass, format!("200 {robots_url}"));
    }

    let txt = String::from_utf8_lossy(&r.body).into_owned();
    let robots = RobotsTxt::parse(&txt);
    // Google は googlebot グループがあればそれだけを見る
    let (ua, rules) = ["googlebot", "*"]
        .into_iter()
        .find_map(|ua| robots.rules_for(ua).map(|r| (ua, r)))
        .unwrap_or(("*", &[]));
    let path = match base.query() {
        Some(q) => format!("{}?{q}", base.path()),
        None => base.path().to_string(),
    };
    let rule = decisive_rule(rules, &path);
    let by = rule.map(ToString::to_string).unwrap_or_default();
    if rule.is_none_or(|r| r.kind == Kind::Allow) {
        s.add(
            "クロール可否",
            Pass,
            format!("許可（User-agent: {ua}）{by}"),
        );
    } else {
        s.add(
            "クロール可否",
            Ng,
            format!("ブロック（User-agent: {ua}）{by}"),
        );
    }

    if robots.sitemaps.is_empty() {
        s.add("Sitemap", Warn, "robots.txt に記載なし");
    }
    let targets: Vec<_> = robots.sitemaps.iter().map(|sm| (sm.clone(), "")).collect();
    for probe in fetch::probe_all(&targets) {
        s.add("Sitemap", probe.level, probe.summary);
    }
    (s, Some(txt))
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
