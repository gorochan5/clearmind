use iced::Color;
use std::ops::Range;

use super::models::ThemePalette;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeKind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Function,
    Type,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseState {
    pub in_block_comment: bool,
    pub in_string: Option<char>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeStack {
    pub scopes: Vec<ScopeKind>,
}

impl ScopeStack {
    pub fn push(&mut self, scope: ScopeKind) {
        self.scopes.push(scope);
    }

    pub fn pop(&mut self) -> Option<ScopeKind> {
        self.scopes.pop()
    }

    pub fn current(&self) -> &ScopeKind {
        self.scopes.last().unwrap_or(&ScopeKind::Plain)
    }
}

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub color: Color,
}

#[derive(Debug, Clone)]
pub struct PaletteHighlighter {
    palette: ThemePalette,
    state: ParseState,
    stack: ScopeStack,
}

impl PaletteHighlighter {
    pub fn new(palette: ThemePalette) -> Self {
        Self {
            palette,
            state: ParseState::default(),
            stack: ScopeStack::default(),
        }
    }

    pub fn seeded(palette: ThemePalette, state: ParseState, stack: ScopeStack) -> Self {
        Self {
            palette,
            state,
            stack,
        }
    }

    pub fn state(&self) -> (&ParseState, &ScopeStack) {
        (&self.state, &self.stack)
    }

    pub fn color_for_scope(&self, scope: &ScopeKind) -> Color {
        match scope {
            ScopeKind::Plain => self.palette.text,
            ScopeKind::Keyword => self.palette.syntax_keyword,
            ScopeKind::String => self.palette.syntax_string,
            ScopeKind::Comment => self.palette.syntax_comment,
            ScopeKind::Number => self.palette.syntax_number,
            ScopeKind::Function => self.palette.syntax_function,
            ScopeKind::Type => self.palette.syntax_type,
        }
    }

    pub fn highlight_line(&mut self, line: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if self.state.in_block_comment {
                let start = i;
                if let Some(end_rel) = line[i..].find("*/") {
                    i += end_rel + 2;
                    self.state.in_block_comment = false;
                } else {
                    i = len;
                }
                spans.push(HighlightSpan {
                    range: start..i,
                    color: self.palette.syntax_comment,
                });
                continue;
            }

            if i + 1 < len && &bytes[i..i + 2] == b"//" {
                spans.push(HighlightSpan {
                    range: i..len,
                    color: self.palette.syntax_comment,
                });
                break;
            }

            if i + 1 < len && &bytes[i..i + 2] == b"/*" {
                let start = i;
                i += 2;
                if let Some(end_rel) = line[i..].find("*/") {
                    i += end_rel + 2;
                } else {
                    self.state.in_block_comment = true;
                    i = len;
                }
                spans.push(HighlightSpan {
                    range: start..i,
                    color: self.palette.syntax_comment,
                });
                continue;
            }

            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                let start = i;
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                spans.push(HighlightSpan {
                    range: start..i,
                    color: self.palette.syntax_string,
                });
                continue;
            }

            if bytes[i].is_ascii_digit() {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.') {
                    i += 1;
                }
                spans.push(HighlightSpan {
                    range: start..i,
                    color: self.palette.syntax_number,
                });
                continue;
            }

            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];
                let color = if is_keyword(word) {
                    self.palette.syntax_keyword
                } else if word.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    self.palette.syntax_type
                } else if i < len && bytes[i] == b'(' {
                    self.palette.syntax_function
                } else {
                    self.color_for_scope(self.stack.current())
                };
                spans.push(HighlightSpan {
                    range: start..i,
                    color,
                });
                continue;
            }

            let start = i;
            i += 1;
            while i < len
                && !bytes[i].is_ascii_alphanumeric()
                && bytes[i] != b'_'
                && bytes[i] != b'"'
                && bytes[i] != b'\''
                && bytes[i] != b'/'
            {
                i += 1;
            }
            spans.push(HighlightSpan {
                range: start..i,
                color: self.palette.text,
            });
        }

        spans
    }
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "let"
            | "mut"
            | "pub"
            | "struct"
            | "enum"
            | "impl"
            | "trait"
            | "for"
            | "in"
            | "while"
            | "loop"
            | "if"
            | "else"
            | "match"
            | "return"
            | "use"
            | "mod"
            | "const"
            | "static"
            | "type"
            | "where"
            | "async"
            | "await"
            | "true"
            | "false"
            | "self"
            | "Self"
            | "def"
            | "class"
            | "import"
            | "from"
            | "function"
            | "var"
            | "export"
    )
}
