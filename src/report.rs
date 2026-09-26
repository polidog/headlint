use std::fmt;

use serde::Serialize;
use unicode_width::UnicodeWidthStr;

/// 判定結果。宣言順が深刻度順なので `max` で悪い方を取れる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    #[serde(rename = "ok")]
    Pass,
    Warn,
    Ng,
}

impl Level {
    pub fn mark(self) -> &'static str {
        match self {
            Level::Pass => "✓",
            Level::Warn => "!",
            Level::Ng => "✗",
            Level::Info => "-",
        }
    }
}

pub const EMPTY: &str = "(なし)";
const LABEL_WIDTH: usize = 22;

#[derive(Debug, Serialize)]
pub struct Item {
    pub label: String,
    pub level: Level,
    pub value: String,
}

impl Item {
    /// 表示幅（全角=2）でラベルを揃える。
    pub fn padded_label(&self) -> String {
        let pad = LABEL_WIDTH.saturating_sub(self.label.width());
        format!("{}{}", self.label, " ".repeat(pad))
    }

    pub fn value_or_empty(&self) -> &str {
        if self.value.is_empty() {
            EMPTY
        } else {
            &self.value
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Section {
    pub name: &'static str,
    pub items: Vec<Item>,
}

impl Section {
    pub fn new(name: &'static str) -> Self {
        Section {
            name,
            items: Vec::new(),
        }
    }

    pub fn add(&mut self, label: impl Into<String>, level: Level, value: impl Into<String>) {
        self.items.push(Item {
            label: label.into(),
            level,
            value: value.into(),
        });
    }

    pub fn count(&self, level: Level) -> usize {
        self.items.iter().filter(|i| i.level == level).count()
    }
}

#[derive(Debug)]
pub struct Report {
    pub url: String,
    pub sections: Vec<Section>,
    /// 取得できた robots.txt の本文
    pub robots_txt: Option<String>,
}

impl Report {
    pub fn count(&self, level: Level) -> usize {
        self.sections.iter().map(|s| s.count(level)).sum()
    }

    pub fn ok(&self) -> bool {
        self.count(Level::Ng) == 0
    }

    pub fn to_json(&self) -> String {
        let v = serde_json::json!({
            "url": self.url,
            "ok": self.ok(),
            "sections": self.sections,
            "robots_txt": self.robots_txt,
        });
        serde_json::to_string_pretty(&v).expect("Report is always serializable") + "\n"
    }
}

/// `--validate` 用のテキスト出力
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for sec in &self.sections {
            writeln!(f, "== {}", sec.name)?;
            for it in &sec.items {
                writeln!(
                    f,
                    "{} {} {}",
                    it.level.mark(),
                    it.padded_label(),
                    it.value_or_empty()
                )?;
            }
        }
        writeln!(
            f,
            "\n✗ {}  ! {}",
            self.count(Level::Ng),
            self.count(Level::Warn)
        )
    }
}
