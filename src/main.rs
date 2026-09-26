mod check;
mod fetch;
mod page;
mod report;
mod robots_txt;
mod tui;

use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "usage: headlint [--json] [--validate] <url>

  (no flags)  show results in the TUI
  --json      print results as JSON to stdout
  --validate  print results as text and exit 1 if any ✗ (can be combined with --json)
  -h, --help  show this help";

enum Output {
    Tui,
    Json,
    Text,
}

struct Args {
    url: String,
    output: Output,
    /// ✗ があれば exit 1
    validate: bool,
}

impl Args {
    /// 失敗時（--help を含む）は usage を出して終了コードを返す。
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, ExitCode> {
        let (mut json, mut validate, mut target) = (false, false, None);
        for a in args {
            match a.as_str() {
                "--json" => json = true,
                "--validate" => validate = true,
                "-h" | "--help" => {
                    println!("{USAGE}");
                    return Err(ExitCode::SUCCESS);
                }
                _ if a.starts_with('-') || target.is_some() => return Err(usage_error()),
                _ => target = Some(a),
            }
        }
        let target = target.ok_or_else(usage_error)?;
        let url = if target.contains("://") {
            target
        } else {
            format!("https://{target}")
        };
        let output = match (json, validate) {
            (true, _) => Output::Json,
            (false, true) => Output::Text,
            (false, false) => Output::Tui,
        };
        Ok(Args {
            url,
            output,
            validate,
        })
    }
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(code) => return code,
    };
    eprintln!("fetching {} ...", args.url);
    let report = match check::run(&args.url) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let out = match args.output {
        Output::Tui => {
            return match tui::run(&report) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            };
        }
        Output::Json => report.to_json(),
        Output::Text => report.to_string(),
    };
    // | head などでパイプが閉じられても panic しないよう書き込みエラーは無視
    let _ = std::io::stdout().write_all(out.as_bytes());
    if args.validate && !report.ok() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
