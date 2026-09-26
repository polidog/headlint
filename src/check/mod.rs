mod favicon;
mod ogp;
mod robots;

use std::thread;

use url::Url;

use crate::fetch;
use crate::page::Page;
use crate::report::Report;

pub fn run(url: &str) -> Result<Report, fetch::Error> {
    let res = fetch::get(url)?;
    let page = Page::parse(&res.body);
    // セクション同士は独立しているので並列に回す
    let (ogp, favicon, (robots, robots_txt)) = thread::scope(|s| {
        let ogp = s.spawn(|| ogp::check(&res.url, &page));
        let favicon = s.spawn(|| favicon::check(&res.url, &page));
        let robots = robots::check(&res, &page);
        (ogp.join().unwrap(), favicon.join().unwrap(), robots)
    });
    Ok(Report {
        url: url.to_string(),
        sections: vec![ogp, favicon, robots],
        robots_txt,
    })
}

/// 相対 URL を絶対 URL に。解決できなければそのまま返す。
fn resolve(base: &Url, r: &str) -> String {
    base.join(r.trim()).map_or(r.to_string(), String::from)
}
