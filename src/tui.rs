use std::io;
use std::ops::ControlFlow;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs};
use ratatui::{DefaultTerminal, Frame};

use crate::report::{EMPTY, Item, Level, Report, Section};

pub fn run(report: &Report) -> io::Result<()> {
    let mut term = ratatui::init();
    let res = App::new(report).run(&mut term);
    ratatui::restore();
    res
}

struct Tab<'a> {
    title: String,
    body: Vec<Line<'a>>,
}

impl<'a> Tab<'a> {
    fn section(sec: &'a Section) -> Self {
        let mut title = sec.name.to_string();
        for lv in [Level::Ng, Level::Warn] {
            match sec.count(lv) {
                0 => {}
                n => title += &format!(" {}{n}", lv.mark()),
            }
        }
        Tab {
            title,
            body: sec.items.iter().map(item_line).collect(),
        }
    }

    fn text(title: &str, text: Option<&'a str>) -> Self {
        let body = match text {
            Some(t) => t.lines().map(Line::from).collect(),
            None => vec![EMPTY.dark_gray().into()],
        };
        Tab {
            title: title.to_string(),
            body,
        }
    }
}

fn item_line(it: &Item) -> Line<'_> {
    let icon = match it.level {
        Level::Pass => it.level.mark().green(),
        Level::Warn => it.level.mark().yellow(),
        Level::Ng => it.level.mark().red(),
        Level::Info => it.level.mark().dark_gray(),
    };
    let value = if it.value.is_empty() {
        EMPTY.dark_gray()
    } else {
        Span::raw(&it.value)
    };
    Line::from(vec![
        icon,
        " ".into(),
        format!("{} ", it.padded_label()).into(),
        value,
    ])
}

struct App<'a> {
    url: &'a str,
    tabs: Vec<Tab<'a>>,
    tab: usize,
    scroll: u16,
}

impl<'a> App<'a> {
    fn new(report: &'a Report) -> Self {
        let mut tabs: Vec<_> = report.sections.iter().map(Tab::section).collect();
        tabs.push(Tab::text("robots.txt", report.robots_txt.as_deref()));
        App {
            url: &report.url,
            tabs,
            tab: 0,
            scroll: 0,
        }
    }

    fn run(mut self, term: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            term.draw(|f| self.draw(f))?;
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
                && self.on_key(k).is_break()
            {
                return Ok(());
            }
        }
    }

    fn draw(&self, f: &mut Frame) {
        use Constraint::{Length, Min};
        let [head, tabs, main, help] =
            Layout::vertical([Length(1), Length(2), Min(0), Length(1)]).areas(f.area());
        f.render_widget(Line::from(self.url), head);
        f.render_widget(
            Tabs::new(self.tabs.iter().map(|t| t.title.as_str()))
                .select(self.tab)
                .highlight_style(Style::new().bold().reversed()),
            tabs,
        );
        f.render_widget(
            Paragraph::new(self.tabs[self.tab].body.clone()).scroll((self.scroll, 0)),
            main,
        );
        f.render_widget(
            Line::from("←/→ tab切替  ↑/↓ スクロール  q 終了").dark_gray(),
            help,
        );
    }

    fn on_key(&mut self, k: KeyEvent) -> ControlFlow<()> {
        let n = self.tabs.len();
        let max = self.tabs[self.tab].body.len().saturating_sub(1) as u16;
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => return ControlFlow::Break(()),
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                return ControlFlow::Break(());
            }
            KeyCode::Right | KeyCode::Tab | KeyCode::Char('l') => self.select((self.tab + 1) % n),
            KeyCode::Left | KeyCode::BackTab | KeyCode::Char('h') => {
                self.select((self.tab + n - 1) % n)
            }
            KeyCode::Down | KeyCode::Char('j') => self.scroll = (self.scroll + 1).min(max),
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll = (self.scroll + 10).min(max),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn select(&mut self, tab: usize) {
        self.tab = tab;
        self.scroll = 0;
    }
}
