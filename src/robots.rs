use std::collections::HashMap;

use regex::Regex;

pub struct Rule {
    pub allow: bool,
    pub path: String,
}

/// robots.txt を User-agent(小文字) ごとのルールと Sitemap に分ける。
pub fn parse_robots(txt: &str) -> (HashMap<String, Vec<Rule>>, Vec<String>) {
    let mut groups: HashMap<String, Vec<Rule>> = HashMap::new();
    let (mut sitemaps, mut agents) = (Vec::new(), Vec::<String>::new());
    let mut in_rules = false;
    for line in txt.lines() {
        let line = line.split('#').next().unwrap_or("");
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim().to_lowercase(), v.trim());
        match k.as_str() {
            "user-agent" => {
                if in_rules {
                    agents.clear();
                    in_rules = false;
                }
                let a = v.to_lowercase();
                groups.entry(a.clone()).or_default();
                agents.push(a);
            }
            "allow" | "disallow" => {
                in_rules = true;
                if v.is_empty() {
                    continue; // 空の Disallow は全許可
                }
                for a in &agents {
                    groups.get_mut(a).unwrap().push(Rule { allow: k == "allow", path: v.to_string() });
                }
            }
            "sitemap" => sitemaps.push(v.to_string()),
            _ => {}
        }
    }
    (groups, sitemaps)
}

fn matches(pattern: &str, path: &str) -> bool {
    let mut re = regex::escape(pattern).replace(r"\*", ".*");
    if let Some(s) = re.strip_suffix(r"\$") {
        re = format!("{s}$");
    }
    Regex::new(&format!("^{re}")).is_ok_and(|r| r.is_match(path))
}

/// Google 方式（最長一致、同長なら Allow 優先）で判定し、効いたルールを返す。
pub fn allowed(rules: &[Rule], path: &str) -> (bool, String) {
    let (mut best, mut allow, mut by) = (None, true, String::new());
    for r in rules.iter().filter(|r| matches(&r.path, path)) {
        let n = r.path.len();
        if best.is_none_or(|b| n > b || n == b && r.allow) {
            best = Some(n);
            allow = r.allow;
            by = format!("{}: {}", if r.allow { "Allow" } else { "Disallow" }, r.path);
        }
    }
    (allow, by)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots() {
        let (groups, sitemaps) = parse_robots(
            "
User-agent: *
Disallow: /admin
Allow: /admin/public
Disallow: /*.pdf$

User-agent: Googlebot
User-agent: Bingbot
Disallow: /private # comment

Sitemap: https://example.com/sitemap.xml
",
        );
        assert_eq!(sitemaps, ["https://example.com/sitemap.xml"]);
        assert_eq!(groups["bingbot"].len(), 1);
        assert_eq!(groups["*"].len(), 3);
        for (path, want) in [
            ("/", true),
            ("/admin/x", false),
            ("/admin/public/a", true),
            ("/docs/a.pdf", false),
            ("/docs/a.pdf?x=1", true),
            ("/private/x", true), // * グループには無い
        ] {
            assert_eq!(allowed(&groups["*"], path).0, want, "{path}");
        }
        assert!(!allowed(&groups["googlebot"], "/private/x").0);
    }
}
