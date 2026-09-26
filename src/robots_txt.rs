use std::collections::HashMap;
use std::fmt;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Allow,
    Disallow,
}

#[derive(Debug)]
pub struct Rule {
    pub kind: Kind,
    pub path: String,
}

impl Rule {
    fn matches(&self, path: &str) -> bool {
        let mut re = regex::escape(&self.path).replace(r"\*", ".*");
        if let Some(s) = re.strip_suffix(r"\$") {
            re = format!("{s}$");
        }
        Regex::new(&format!("^{re}")).is_ok_and(|r| r.is_match(path))
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.path)
    }
}

#[derive(Debug, Default)]
pub struct RobotsTxt {
    /// key: User-agent（小文字）
    groups: HashMap<String, Vec<Rule>>,
    pub sitemaps: Vec<String>,
}

impl RobotsTxt {
    pub fn parse(txt: &str) -> Self {
        let mut r = RobotsTxt::default();
        let mut agents = Vec::<String>::new();
        let mut in_rules = false;
        for line in txt.lines() {
            let line = line.split('#').next().unwrap_or("");
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let (k, v) = (k.trim().to_lowercase(), v.trim());
            let kind = match k.as_str() {
                "user-agent" => {
                    if in_rules {
                        agents.clear();
                        in_rules = false;
                    }
                    let a = v.to_lowercase();
                    r.groups.entry(a.clone()).or_default();
                    agents.push(a);
                    continue;
                }
                "sitemap" => {
                    r.sitemaps.push(v.to_string());
                    continue;
                }
                "allow" => Kind::Allow,
                "disallow" => Kind::Disallow,
                _ => continue,
            };
            in_rules = true;
            if v.is_empty() {
                continue; // 空の Disallow は全許可
            }
            for a in &agents {
                r.groups.get_mut(a).unwrap().push(Rule {
                    kind,
                    path: v.to_string(),
                });
            }
        }
        r
    }

    pub fn rules_for(&self, agent: &str) -> Option<&[Rule]> {
        self.groups.get(agent).map(Vec::as_slice)
    }
}

/// Google 方式（最長一致、同長なら Allow 優先）で効くルールを返す。None なら許可。
pub fn decisive_rule<'a>(rules: &'a [Rule], path: &str) -> Option<&'a Rule> {
    rules
        .iter()
        .filter(|r| r.matches(path))
        .max_by_key(|r| (r.path.len(), r.kind == Kind::Allow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots() {
        let r = RobotsTxt::parse(
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
        let allowed = |ua, path| {
            decisive_rule(r.rules_for(ua).unwrap(), path).is_none_or(|r| r.kind == Kind::Allow)
        };
        assert_eq!(r.sitemaps, ["https://example.com/sitemap.xml"]);
        assert_eq!(r.rules_for("bingbot").unwrap().len(), 1);
        assert_eq!(r.rules_for("*").unwrap().len(), 3);
        for (path, want) in [
            ("/", true),
            ("/admin/x", false),
            ("/admin/public/a", true),
            ("/docs/a.pdf", false),
            ("/docs/a.pdf?x=1", true),
            ("/private/x", true), // * グループには無い
        ] {
            assert_eq!(allowed("*", path), want, "{path}");
        }
        assert!(!allowed("googlebot", "/private/x"));
    }
}
