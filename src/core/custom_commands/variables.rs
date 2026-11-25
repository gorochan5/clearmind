use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedTemplate {
    pub text: String,
    pub cursor_offset: Option<usize>,
}

pub struct VariableContext<'a> {
    pub filename: Option<&'a str>,
    pub clipboard: Option<&'a str>,
    pub arg: Option<&'a str>,
}

pub fn expand_template(template: &str, ctx: &VariableContext<'_>) -> ExpandedTemplate {
    let (date, time) = current_date_time_strings();
    let datetime = format!("{} {}", date, time);
    let filename = ctx.filename.unwrap_or("Untitled");
    let clipboard = ctx.clipboard.unwrap_or("");
    let arg = ctx.arg.unwrap_or("");

    let replaced = template
        .replace("{{date}}", &date)
        .replace("{{time}}", &time)
        .replace("{{datetime}}", &datetime)
        .replace("{{filename}}", filename)
        .replace("{{clipboard}}", clipboard)
        .replace("{{arg}}", arg);

    if let Some(pos) = replaced.find("{{cursor}}") {
        let text = replaced.replacen("{{cursor}}", "", 1);
        ExpandedTemplate {
            text,
            cursor_offset: Some(pos),
        }
    } else {
        ExpandedTemplate {
            text: replaced,
            cursor_offset: None,
        }
    }
}

fn current_date_time_strings() -> (String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let min = (rem % 3_600) / 60;
    let (year, month, day) = civil_from_days(days as i64);
    (
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{hour:02}:{min:02}"),
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_variables_and_extracts_cursor() {
        let ctx = VariableContext {
            filename: Some("notes.md"),
            clipboard: Some("hello"),
            arg: Some("rust"),
        };
        let res = expand_template("File: {{filename}} Clip: {{clipboard}} -> {{cursor}}!", &ctx);
        assert_eq!(res.text, "File: notes.md Clip: hello -> !");
        assert_eq!(res.cursor_offset, Some("File: notes.md Clip: hello -> ".len()));
    }
}
