use super::models::{CustomCommand, SlashSuggestion, TriggerContext};
use super::variables::{ExpandedTemplate, VariableContext, expand_template};

pub fn detect_trigger_context(
    line_before_cursor: &str,
    in_fenced_code: bool,
) -> TriggerContext {
    if in_fenced_code {
        return TriggerContext::Suppressed;
    }

    let backticks = line_before_cursor.matches('`').count();
    if backticks % 2 == 1 {
        return TriggerContext::Suppressed;
    }

    let last_token = line_before_cursor.split_whitespace().last().unwrap_or("");
    if last_token.starts_with("http://") || last_token.starts_with("https://") {
        return TriggerContext::Suppressed;
    }

    // Count unescaped single `$` delimiters (ignoring `$$` pairs)
    if has_unclosed_inline_math(line_before_cursor) {
        return TriggerContext::Math;
    }

    TriggerContext::Prose
}

/// Document-aware context detection: accounts for multi-line fenced code blocks
/// (` ``` ` / `~~~`), multi-line `$$ ... $$` display math blocks, single-line `$$ ... $$`,
/// inline `$ ... $`, inline backtick spans, and URLs.
pub fn detect_document_trigger_context(
    lines: &[String],
    cursor_line: usize,
    cursor_col: usize,
) -> TriggerContext {
    let mut fence: Option<(char, usize)> = None;
    let mut in_display_math = false;

    for raw in lines.iter().take(cursor_line) {
        let trimmed = raw.trim();
        if let Some((fch, flen)) = fence {
            if is_closing_fence(trimmed, fch, flen) {
                fence = None;
            }
            continue;
        }
        if !in_display_math && let Some((fch, flen)) = parse_opening_fence(trimmed) {
            fence = Some((fch, flen));
            continue;
        }
        if in_display_math {
            if trimmed.ends_with("$$") {
                in_display_math = false;
            }
        } else if let Some(after) = trimmed.strip_prefix("$$") {
            let is_single_line = after.trim_end().ends_with("$$") && after.trim().len() >= 2;
            if !is_single_line {
                in_display_math = true;
            }
        }
    }

    if fence.is_some() {
        return TriggerContext::Suppressed;
    }

    let cur_raw = lines
        .get(cursor_line)
        .map(|s| s.trim_end_matches(['\r', '\n']))
        .unwrap_or("");
    let before: String = cur_raw.chars().take(cursor_col).collect();

    if in_display_math {
        // Still in math unless cursor is right after the closing `$$`
        if before.trim().ends_with("$$") {
            return TriggerContext::Prose;
        }
        return TriggerContext::Math;
    }

    // Single-line `$$ ... $$` where cursor sits between the opening and closing `$$`
    let after: String = cur_raw.chars().skip(cursor_col).collect();
    if before.contains("$$") {
        let open_count = before.matches("$$").count();
        if open_count % 2 == 1 {
            return TriggerContext::Math;
        }
        if after.contains("$$") && open_count > 0 && before.trim_start().starts_with("$$") {
            return TriggerContext::Math;
        }
    }

    detect_trigger_context(&before, false)
}

fn parse_opening_fence(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == first).count();
    if count >= 3 {
        Some((first, count))
    } else {
        None
    }
}

fn is_closing_fence(trimmed: &str, ch: char, min_len: usize) -> bool {
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    count >= min_len && trimmed[count..].trim().is_empty()
}

fn has_unclosed_inline_math(before: &str) -> bool {
    let chars: Vec<char> = before.chars().collect();
    let mut single_dollars = 0usize;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == '$' {
            if i + 1 < chars.len() && chars[i + 1] == '$' {
                i += 2;
                continue;
            }
            single_dollars += 1;
        }
        i += 1;
    }
    single_dollars % 2 == 1
}

pub fn builtin_prose_commands() -> Vec<SlashSuggestion> {
    vec![
        SlashSuggestion {
            trigger: "code".into(),
            label: "Code Block".into(),
            description: "Insert fenced code block (/code [lang])".into(),
            template: "```{{arg}}\n{{cursor}}\n```".into(),
            takes_args: true,
        },
        SlashSuggestion {
            trigger: "latex".into(),
            label: "LaTeX Block".into(),
            description: "Insert display math block ($$...$$)".into(),
            template: "$$\n{{cursor}}\n$$".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "table".into(),
            label: "Table".into(),
            description: "Insert Markdown table (/table [RxC])".into(),
            template: "| Col 1 | Col 2 |\n| :--- | :--- |\n| {{cursor}} |  |\n|  |  |".into(),
            takes_args: true,
        },
        SlashSuggestion {
            trigger: "mermaid".into(),
            label: "Mermaid Diagram".into(),
            description: "Insert Mermaid diagram (/mermaid [type])".into(),
            template: "```mermaid\nflowchart TD\n    {{cursor}}\n```".into(),
            takes_args: true,
        },
        SlashSuggestion {
            trigger: "callout".into(),
            label: "Callout".into(),
            description: "Insert GitHub alert (/callout [note|tip|warning])".into(),
            template: "> [!NOTE]\n> {{cursor}}".into(),
            takes_args: true,
        },
        SlashSuggestion {
            trigger: "task".into(),
            label: "Task Item".into(),
            description: "Insert task list checkbox".into(),
            template: "- [ ] {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "badge".into(),
            label: "Badge".into(),
            description: "Insert inline badge".into(),
            template: "![badge](https://img.shields.io/badge/{{cursor}}-blue)".into(),
            takes_args: false,
        },
    ]
}

pub fn builtin_math_commands() -> Vec<SlashSuggestion> {
    vec![
        SlashSuggestion {
            trigger: "frac".into(),
            label: "Fraction".into(),
            description: "\\frac{a}{b}".into(),
            template: "\\frac{{{cursor}}}{}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "sum".into(),
            label: "Summation".into(),
            description: "\\sum_{i=1}^{n}".into(),
            template: "\\sum_{{{cursor}}}^{}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "int".into(),
            label: "Integral".into(),
            description: "\\int_{a}^{b}".into(),
            template: "\\int_{{{cursor}}}^{}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "prod".into(),
            label: "Product".into(),
            description: "\\prod_{i=1}^{n}".into(),
            template: "\\prod_{{{cursor}}}^{}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "lim".into(),
            label: "Limit".into(),
            description: "\\lim_{x \\to \\infty}".into(),
            template: "\\lim_{{{cursor}}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "sqrt".into(),
            label: "Square Root".into(),
            description: "\\sqrt{x}".into(),
            template: "\\sqrt{{{cursor}}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "matrix".into(),
            label: "Matrix".into(),
            description: "\\begin{matrix}...\\end{matrix}".into(),
            template: "\\begin{matrix}\n{{cursor}} & \\\\\n & \n\\end{matrix}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "pmatrix".into(),
            label: "Parenthesized Matrix".into(),
            description: "\\begin{pmatrix}...\\end{pmatrix}".into(),
            template: "\\begin{pmatrix}\n{{cursor}} & \\\\\n & \n\\end{pmatrix}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "bmatrix".into(),
            label: "Bracketed Matrix".into(),
            description: "\\begin{bmatrix}...\\end{bmatrix}".into(),
            template: "\\begin{bmatrix}\n{{cursor}} & \\\\\n & \n\\end{bmatrix}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "cases".into(),
            label: "Piecewise Cases".into(),
            description: "\\begin{cases}...\\end{cases}".into(),
            template: "\\begin{cases}\n{{cursor}} & \\text{if } \\\\\n & \\text{otherwise}\n\\end{cases}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "vec".into(),
            label: "Vector".into(),
            description: "\\vec{x}".into(),
            template: "\\vec{{{cursor}}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "hat".into(),
            label: "Hat Accent".into(),
            description: "\\hat{x}".into(),
            template: "\\hat{{{cursor}}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "text".into(),
            label: "Text in Math".into(),
            description: "\\text{...}".into(),
            template: "\\text{{{cursor}}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "alpha".into(),
            label: "Alpha (α)".into(),
            description: "\\alpha".into(),
            template: "\\alpha{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "beta".into(),
            label: "Beta (β)".into(),
            description: "\\beta".into(),
            template: "\\beta{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "gamma".into(),
            label: "Gamma (γ)".into(),
            description: "\\gamma".into(),
            template: "\\gamma{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "delta".into(),
            label: "Delta (δ)".into(),
            description: "\\delta".into(),
            template: "\\delta{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "theta".into(),
            label: "Theta (θ)".into(),
            description: "\\theta".into(),
            template: "\\theta{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "lambda".into(),
            label: "Lambda (λ)".into(),
            description: "\\lambda".into(),
            template: "\\lambda{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "pi".into(),
            label: "Pi (π)".into(),
            description: "\\pi".into(),
            template: "\\pi{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "sigma".into(),
            label: "Sigma (σ)".into(),
            description: "\\sigma".into(),
            template: "\\sigma{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "omega".into(),
            label: "Omega (ω)".into(),
            description: "\\omega".into(),
            template: "\\omega{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "infty".into(),
            label: "Infinity (∞)".into(),
            description: "\\infty".into(),
            template: "\\infty{{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "pm".into(),
            label: "Plus-Minus (±)".into(),
            description: "\\pm".into(),
            template: "\\pm {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "times".into(),
            label: "Times (×)".into(),
            description: "\\times".into(),
            template: "\\times {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "cdot".into(),
            label: "Center Dot (·)".into(),
            description: "\\cdot".into(),
            template: "\\cdot {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "leq".into(),
            label: "Less or Equal (≤)".into(),
            description: "\\leq".into(),
            template: "\\leq {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "geq".into(),
            label: "Greater or Equal (≥)".into(),
            description: "\\geq".into(),
            template: "\\geq {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "neq".into(),
            label: "Not Equal (≠)".into(),
            description: "\\neq".into(),
            template: "\\neq {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "approx".into(),
            label: "Approximately (≈)".into(),
            description: "\\approx".into(),
            template: "\\approx {{cursor}}".into(),
            takes_args: false,
        },
        SlashSuggestion {
            trigger: "to".into(),
            label: "Right Arrow (→)".into(),
            description: "\\to".into(),
            template: "\\to {{cursor}}".into(),
            takes_args: false,
        },
    ]
}

pub fn suggestions_for_context(
    ctx: TriggerContext,
    query: &str,
    user_commands: &[CustomCommand],
) -> Vec<SlashSuggestion> {
    let q = query.trim().to_lowercase();
    let mut items = match ctx {
        TriggerContext::Suppressed => return Vec::new(),
        TriggerContext::Math => builtin_math_commands(),
        TriggerContext::Prose => {
            let mut list = builtin_prose_commands();
            for cmd in user_commands {
                list.push(SlashSuggestion {
                    trigger: cmd.trigger.clone(),
                    label: cmd.label.clone(),
                    description: cmd.description.clone(),
                    template: cmd.template.clone(),
                    takes_args: false,
                });
            }
            list
        }
    };

    if !q.is_empty() {
        items.retain(|s| {
            s.trigger.to_lowercase().contains(&q)
                || s.label.to_lowercase().contains(&q)
                || s.description.to_lowercase().contains(&q)
        });
        items.sort_by_key(|s| {
            let trig = s.trigger.to_lowercase();
            let lbl = s.label.to_lowercase();
            if trig == q {
                0
            } else if trig.starts_with(&q) {
                1
            } else if lbl.starts_with(&q) {
                2
            } else {
                3
            }
        });
    }
    items
}

/// Expands a built-in or user slash command template, supporting dynamic inline args
/// such as `/table 3x4`, `/mermaid sequence`, `/callout warning`, and `/code rust`.
pub fn expand_suggestion(
    suggestion: &SlashSuggestion,
    ctx: &VariableContext<'_>,
) -> ExpandedTemplate {
    let arg = ctx.arg.unwrap_or("").trim();

    if suggestion.trigger == "table"
        && !arg.is_empty()
        && let Some((rows, cols)) = parse_table_dimensions(arg)
    {
        let headers = (1..=cols)
            .map(|c| format!("Col {c}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let delim = vec![":---"; cols].join(" | ");
        let mut lines = vec![format!("| {headers} |"), format!("| {delim} |")];
        for r in 0..rows {
            let cells = (0..cols)
                .map(|c| {
                    if r == 0 && c == 0 {
                        "{{cursor}}".to_string()
                    } else {
                        " ".to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ");
            lines.push(format!("| {cells} |"));
        }
        let template = lines.join("\n");
        return expand_template(&template, ctx);
    }

    if suggestion.trigger == "mermaid" && !arg.is_empty() {
        let lower = arg.to_ascii_lowercase();
        let header = match lower.as_str() {
            "seq" | "sequence" => "sequenceDiagram",
            "class" => "classDiagram",
            "state" => "stateDiagram-v2",
            "er" => "erDiagram",
            "mindmap" => "mindmap",
            "gantt" => "gantt",
            "pie" => "pie",
            _ => "flowchart TD",
        };
        let template = format!("```mermaid\n{header}\n    {{{{cursor}}}}\n```");
        return expand_template(&template, ctx);
    }

    if suggestion.trigger == "callout" && !arg.is_empty() {
        let kind = match arg.to_ascii_lowercase().as_str() {
            "tip" => "TIP",
            "important" | "imp" => "IMPORTANT",
            "warn" | "warning" => "WARNING",
            "caution" | "danger" => "CAUTION",
            _ => "NOTE",
        };
        let template = format!("> [!{kind}]\n> {{{{cursor}}}}");
        return expand_template(&template, ctx);
    }

    expand_template(&suggestion.template, ctx)
}

fn parse_table_dimensions(arg: &str) -> Option<(usize, usize)> {
    let lower = arg.to_ascii_lowercase();
    let (r_str, c_str) = if let Some(pair) = lower.split_once('x') {
        pair
    } else if let Some(pair) = lower.split_once('*') {
        pair
    } else {
        let n = lower.parse::<usize>().ok()?;
        return Some((n.clamp(1, 20), n.clamp(1, 12)));
    };
    let rows = r_str.trim().parse::<usize>().ok()?.clamp(1, 20);
    let cols = c_str.trim().parse::<usize>().ok()?.clamp(1, 12);
    Some((rows, cols))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_context_detects_suppressed_math_and_prose() {
        assert_eq!(
            detect_trigger_context("hello ", true),
            TriggerContext::Suppressed
        );
        assert_eq!(
            detect_trigger_context("inline `code /", false),
            TriggerContext::Suppressed
        );
        assert_eq!(
            detect_trigger_context("https://example.com/", false),
            TriggerContext::Suppressed
        );
        assert_eq!(
            detect_trigger_context("let $x = /", false),
            TriggerContext::Math
        );
        assert_eq!(
            detect_trigger_context("normal prose /", false),
            TriggerContext::Prose
        );
    }

    #[test]
    fn detects_multiline_display_math_block() {
        let lines = vec!["$$".to_string(), "/".to_string(), "$$".to_string()];
        assert_eq!(
            detect_document_trigger_context(&lines, 1, 1),
            TriggerContext::Math
        );
    }
}
