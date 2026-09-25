mod check;
mod robots;

use check::{Level, Section};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs};
use unicode_width::UnicodeWidthStr;

fn icon(lv: Level) -> Span<'static> {
    match lv {
        Level::Pass => "✓".green(),
        Level::Warn => "!".yellow(),
        Level::Ng => "✗".red(),
        Level::Info => "-".dark_gray(),
    }
}

fn lines(sec: &Section) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    for it in &sec.items {
        if it.label.is_empty() {
            out.extend(it.value.lines().map(Line::from));
            continue;
        }
        let pad = " ".repeat(22usize.saturating_sub(it.label.width()));
        let value = if it.value.is_empty() { "(なし)".dark_gray() } else { Span::raw(&it.value) };
        out.push(Line::from(vec![icon(it.lv), " ".into(), format!("{}{pad} ", it.label).into(), value]));
    }
    out
}

fn tab_title(sec: &Section) -> String {
    let count = |lv| sec.items.iter().filter(|i| i.lv == lv).count();
    let mut t = sec.name.to_string();
    if count(Level::Ng) > 0 {
        t += &format!(" ✗{}", count(Level::Ng));
    }
    if count(Level::Warn) > 0 {
        t += &format!(" !{}", count(Level::Warn));
    }
    t
}

fn text(secs: &[Section]) -> String {
    use std::fmt::Write;
    let (mut out, mut ng, mut warn) = (String::new(), 0, 0);
    for sec in secs.iter().filter(|s| s.items.iter().any(|i| !i.label.is_empty())) {
        writeln!(out, "== {}", sec.name).unwrap();
        for it in sec.items.iter().filter(|i| !i.label.is_empty()) {
            let mark = icon(it.lv).content;
            ng += (it.lv == Level::Ng) as usize;
            warn += (it.lv == Level::Warn) as usize;
            let pad = " ".repeat(22usize.saturating_sub(it.label.width()));
            let v = if it.value.is_empty() { "(なし)" } else { &it.value };
            writeln!(out, "{mark} {}{pad} {v}", it.label).unwrap();
        }
    }
    writeln!(out, "\n✗ {ng}  ! {warn}").unwrap();
    out
}

fn run(term: &mut ratatui::DefaultTerminal, url: &str, secs: &[Section]) -> std::io::Result<()> {
    let (mut tab, mut scroll) = (0usize, 0u16);
    loop {
        let body = lines(&secs[tab]);
        let max = body.len().saturating_sub(1) as u16;
        term.draw(|f| {
            use Constraint::{Length, Min};
            let [head, tabs, main, help] = Layout::vertical([Length(1), Length(2), Min(0), Length(1)]).areas(f.area());
            f.render_widget(Line::from(url), head);
            f.render_widget(
                Tabs::new(secs.iter().map(tab_title)).select(tab).highlight_style(Style::new().bold().reversed()),
                tabs,
            );
            f.render_widget(Paragraph::new(body).scroll((scroll, 0)), main);
            f.render_widget(Line::from("←/→ tab切替  ↑/↓ スクロール  q 終了").dark_gray(), help);
        })?;
        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => (tab, scroll) = ((tab + 1) % secs.len(), 0),
            KeyCode::Left | KeyCode::BackTab | KeyCode::Char('h') => {
                (tab, scroll) = ((tab + secs.len() - 1) % secs.len(), 0)
            }
            KeyCode::Down | KeyCode::Char('j') => scroll = (scroll + 1).min(max),
            KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
            KeyCode::PageDown => scroll = (scroll + 10).min(max),
            KeyCode::PageUp => scroll = scroll.saturating_sub(10),
            _ => {}
        }
    }
}

const USAGE: &str = "usage: seo-checker [--json] [--validate] <url>

  (なし)      TUI で表示
  --json      結果を JSON で標準出力
  --validate  結果をテキストで出力し、✗ が1つでもあれば exit 1（--json と併用可）";

fn main() {
    let (mut json, mut validate, mut target) = (false, false, None);
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--json" => json = true,
            "--validate" => validate = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ if a.starts_with('-') || target.is_some() => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
            _ => target = Some(a),
        }
    }
    let Some(target) = target else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let raw = if target.contains("://") { target } else { format!("https://{target}") };
    eprintln!("fetching {raw} ...");
    let secs = check::check(&raw).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let failed = secs.iter().flat_map(|s| &s.items).any(|i| i.lv == Level::Ng);

    if json || validate {
        let out = if json {
            let v = serde_json::json!({ "url": raw, "ok": !failed, "sections": secs });
            serde_json::to_string_pretty(&v).unwrap() + "\n"
        } else {
            text(&secs)
        };
        // | head などでパイプが閉じられても panic しないよう書き込みエラーは無視
        let _ = std::io::Write::write_all(&mut std::io::stdout(), out.as_bytes());
        std::process::exit(if validate && failed { 1 } else { 0 });
    }

    let mut term = ratatui::init();
    let res = run(&mut term, &raw, &secs);
    ratatui::restore();
    if let Err(e) = res {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
