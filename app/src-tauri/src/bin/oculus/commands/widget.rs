//! `oculus widget check`: `widgets::check` over one file, printed one
//! finding a line.

use crate::*;
use app_lib::widgets::check::{check, Finding};

impl Ctx {
    pub(crate) fn widget(&self, action: WidgetAction) -> Result<(), String> {
        match action {
            WidgetAction::Check(a) => self.widget_check(&a),
        }
    }

    fn widget_check(&self, args: &WidgetCheckArgs) -> Result<(), String> {
        let source = read_input(&args.file, "widget")?;
        let name = if args.file == "-" {
            "stdin"
        } else {
            &args.file
        };
        let findings = check(&source);
        if self.json {
            self.emit(&findings)?;
        } else if findings.is_empty() {
            println!("{} {name}", paint("no problems", GREEN));
        } else {
            for f in &findings {
                println!("{}", line(name, f));
            }
        }
        match findings.len() {
            0 => Ok(()),
            1 => Err(format!("1 problem in {name}")),
            n => Err(format!("{n} problems in {name}")),
        }
    }
}

/// `file:line:col rule: message`, or `file: rule: message` with no place.
fn line(name: &str, f: &Finding) -> String {
    let place = match (f.line, f.col) {
        (Some(l), Some(c)) => format!("{name}:{l}:{c}"),
        _ => name.to_string(),
    };
    format!("{place} {}: {}", paint(f.rule, YELLOW), f.message)
}
