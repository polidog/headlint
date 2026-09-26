use std::collections::HashMap;

use scraper::{Html, Selector};

pub struct Link {
    /// 小文字化済み
    pub rel: String,
    pub href: String,
    pub sizes: String,
}

/// HTML から SEO チェックに要る部分だけ抜き出したもの。
pub struct Page {
    pub title: String,
    /// key: property / name（小文字）。同じ key は先勝ち。
    meta: HashMap<String, String>,
    pub links: Vec<Link>,
}

impl Page {
    pub fn parse(body: &[u8]) -> Self {
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
            let k = v
                .attr("property")
                .filter(|s| !s.is_empty())
                .or(v.attr("name"))
                .unwrap_or("")
                .to_lowercase();
            if !k.is_empty() {
                meta.entry(k)
                    .or_insert_with(|| v.attr("content").unwrap_or("").to_string());
            }
        }
        let links = doc
            .select(&sel("link"))
            .map(|e| {
                let a = |k| e.value().attr(k).unwrap_or("").to_string();
                Link {
                    rel: a("rel").to_lowercase(),
                    href: a("href"),
                    sizes: a("sizes"),
                }
            })
            .collect();
        Page { title, meta, links }
    }

    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(String::as_str)
    }

    pub fn link(&self, rel: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.rel == rel)
    }
}
