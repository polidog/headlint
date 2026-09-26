use std::fmt;
use std::io::{self, Read};
use std::net::ToSocketAddrs;
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;

use url::Url;

use crate::report::Level;

/// これ以上は読まない。X のカード画像上限（5MB）も兼ねる。
pub const MAX_BODY: usize = 5 << 20;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (compatible; headlint/0.1)")
        // ureq 2 は Happy Eyeballs が無く、IPv6 の SYN が落ちる環境だと並列接続で 1〜4 秒詰まる。
        // IPv4 を先に試す（IPv6 は後ろに残すので IPv6 only のホストにも繋がる）
        .resolver(|addr: &str| {
            let mut addrs: Vec<_> = addr.to_socket_addrs()?.collect();
            addrs.sort_by_key(|a| a.is_ipv6());
            Ok(addrs)
        })
        .build()
});

#[derive(Debug)]
pub enum Error {
    Http(Box<ureq::Error>),
    Url(url::ParseError),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Http(e) => e.fmt(f),
            Error::Url(e) => e.fmt(f),
            Error::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Error::Url(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub struct Response {
    pub status: u16,
    /// リダイレクト後の URL
    pub url: Url,
    pub content_type: String,
    pub x_robots: Vec<String>,
    pub body: Vec<u8>,
}

/// 4xx/5xx も Err にせず Response として返す。
pub fn get(u: &str) -> Result<Response, Error> {
    let resp = match AGENT.get(u).call() {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(Error::Http(Box::new(e))),
    };
    let url = Url::parse(resp.get_url())?;
    let status = resp.status();
    let content_type = resp.header("Content-Type").unwrap_or_default().to_string();
    let x_robots = resp
        .all("X-Robots-Tag")
        .into_iter()
        .map(String::from)
        .collect();
    let mut body = Vec::new();
    resp.into_reader()
        .take(MAX_BODY as u64)
        .read_to_end(&mut body)?;
    Ok(Response {
        status,
        url,
        content_type,
        x_robots,
        body,
    })
}

pub struct Probe {
    pub level: Level,
    /// "URL  (200 image/png 1.2KB)" 形式
    pub summary: String,
    pub body: Vec<u8>,
}

/// URL を取得し、200 かつ Content-Type が `want_type` で始まれば Pass。
pub fn probe(u: &str, want_type: &str) -> Probe {
    let r = match get(u) {
        Ok(r) => r,
        Err(e) => {
            return Probe {
                level: Level::Ng,
                summary: format!("{u}  ({e})"),
                body: Vec::new(),
            };
        }
    };
    let summary = format!(
        "{u}  ({} {} {:.1}KB)",
        r.status,
        r.content_type,
        r.body.len() as f64 / 1024.0
    );
    let level = if r.status != 200 {
        Level::Ng
    } else if !r.content_type.starts_with(want_type) {
        Level::Warn
    } else {
        Level::Pass
    };
    Probe {
        level,
        summary,
        body: r.body,
    }
}

/// `(url, want_type)` をまとめて並列に probe する。結果は入力と同じ順。
// ponytail: 1 件 1 スレッド。1 ページの画像・favicon 程度なら十分、数百件になるなら上限付きプールに
pub fn probe_all(targets: &[(String, &str)]) -> Vec<Probe> {
    thread::scope(|s| {
        let handles: Vec<_> = targets
            .iter()
            .map(|(u, want)| s.spawn(move || probe(u, want)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}
