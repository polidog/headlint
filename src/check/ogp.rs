use unicode_width::UnicodeWidthStr;
use url::Url;

use super::resolve;
use crate::fetch::{self, MAX_BODY};
use crate::page::Page;
use crate::report::Level::{self, *};
use crate::report::Section;

// 表示幅（全角=2）で数える。Google の検索結果で切れずに出るのは概ね幅60（全角30字）まで。
const TITLE_MIN_WIDTH: usize = 20;
const TITLE_MAX_WIDTH: usize = 60;

/// (key, 無いときの判定)
const TAGS: &[(&str, Level)] = &[
    ("og:title", Ng),
    ("og:type", Ng),
    ("og:url", Ng),
    ("og:image", Ng),
    ("og:description", Warn),
    ("og:site_name", Info),
    ("og:locale", Info),
    ("og:image:width", Info),
    ("og:image:height", Info),
    ("og:image:alt", Info),
    ("twitter:card", Warn),
    ("twitter:site", Info),
    ("twitter:title", Info),
    ("twitter:description", Info),
    ("twitter:image", Info),
];

pub fn check(base: &Url, p: &Page) -> Section {
    let mut s = Section::new("OGP");
    let (lv, v) = title_check(&p.title);
    s.add("title", lv, v);
    match p.meta("description") {
        Some(d) => s.add("description", Pass, counted(d)),
        None => s.add("description", Warn, ""),
    }
    for &(k, missing) in TAGS {
        match p.meta(k) {
            Some(v) => s.add(k, Pass, v),
            None => s.add(k, missing, ""),
        }
    }
    let images: Vec<_> = ["og:image", "twitter:image"]
        .into_iter()
        .filter_map(|k| p.meta(k).filter(|v| !v.is_empty()).map(|v| (k, v)))
        .collect();
    let targets: Vec<_> = images
        .iter()
        .map(|(_, v)| (resolve(base, v), "image/"))
        .collect();
    for ((k, v), probe) in images.into_iter().zip(fetch::probe_all(&targets)) {
        if !v.starts_with("http") {
            s.add(format!("{k} 形式"), Warn, "相対URL（絶対URLにすべき）");
        }
        let ok = probe.level != Ng;
        s.add(format!("{k} 取得"), probe.level, probe.summary);
        if k == "og:image" && ok {
            image_size(&mut s, p, &probe.body);
        }
    }
    s
}

fn counted(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    format!("{s}  [{}文字]", s.chars().count())
}

fn title_check(t: &str) -> (Level, String) {
    if t.is_empty() {
        return (Ng, String::new());
    }
    let w = t.width();
    let v = format!("{t}  [{}文字・幅{w}]", t.chars().count());
    let hint = format!(
        "（目安: 幅{TITLE_MIN_WIDTH}〜{TITLE_MAX_WIDTH}＝全角{}〜{}字）",
        TITLE_MIN_WIDTH / 2,
        TITLE_MAX_WIDTH / 2
    );
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
        lv = lv.max(Warn);
        notes.push("比率が 1.91:1 でない（切り抜かれる）");
    }
    (
        lv,
        format!("{w}x{h}（{ratio:.2}:1）  {}", notes.join(" / "))
            .trim_end()
            .to_string(),
    )
}

fn image_size(s: &mut Section, p: &Page, body: &[u8]) {
    let size = match imagesize::blob_size(body) {
        Ok(size) => size,
        Err(e) => return s.add("og:image サイズ", Warn, format!("読み取れない（{e}）")),
    };
    let (mut lv, mut v) = image_check(size.width, size.height);
    if body.len() >= MAX_BODY {
        lv = lv.max(Warn);
        v += " / 5MB 以上（X で表示されない）";
    }
    s.add("og:image サイズ", lv, v);
    // og:image:width/height の宣言値と実寸の食い違い
    for (k, actual) in [
        ("og:image:width", size.width),
        ("og:image:height", size.height),
    ] {
        if let Some(d) = p.meta(k).and_then(|v| v.trim().parse::<usize>().ok())
            && d != actual
        {
            s.add(
                format!("{k} 宣言値"),
                Warn,
                format!("{d} だが実寸は {actual}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title() {
        assert_eq!(title_check(""), (Ng, String::new()));
        assert_eq!(title_check("短い").0, Warn);
        assert_eq!(
            title_check("ちょうどいい長さのタイトルです｜サイト名").0,
            Pass
        ); // 幅40
        assert_eq!(title_check(&"あ".repeat(31)).0, Warn); // 幅62
    }

    #[test]
    fn image() {
        assert_eq!(image_check(1200, 630).0, Pass);
        assert_eq!(image_check(2400, 1260).0, Pass);
        assert_eq!(image_check(800, 419).0, Warn); // 比率OK・小さい
        assert_eq!(image_check(1200, 1200).0, Warn); // 正方形
        assert_eq!(image_check(150, 150).0, Ng);
    }
}
