use url::Url;

use super::resolve;
use crate::fetch;
use crate::page::Page;
use crate::report::Level::*;
use crate::report::Section;

pub fn check(base: &Url, p: &Page) -> Section {
    let mut s = Section::new("Favicon");
    // (ラベル, URL, 期待する Content-Type)
    let mut targets = Vec::new();
    for l in &p.links {
        if l.rel.contains("icon") {
            let label = if l.sizes.is_empty() {
                l.rel.clone()
            } else {
                format!("{} {}", l.rel, l.sizes)
            };
            targets.push((label, resolve(base, &l.href), "image/"));
        } else if l.rel == "manifest" {
            targets.push(("manifest".to_string(), resolve(base, &l.href), ""));
        }
    }
    let found = p.links.iter().any(|l| l.rel.contains("icon"));
    targets.push((
        "/favicon.ico".to_string(),
        resolve(base, "/favicon.ico"),
        "image/",
    ));

    let urls: Vec<_> = targets
        .iter()
        .map(|(_, u, want)| (u.clone(), *want))
        .collect();
    let mut probes = fetch::probe_all(&urls);
    let ico = probes.pop().expect("/favicon.ico は必ず入れている");
    for ((label, _, _), probe) in targets.into_iter().zip(probes) {
        s.add(label, probe.level, probe.summary);
    }
    if !found {
        s.add("link rel=icon", Warn, "");
    }
    // link で指定済みなら無くても致命的ではない
    s.add("/favicon.ico", ico.level.min(Warn), ico.summary);
    s
}
