use crate::database::sql::{CompactableInsert, SqlImportParser};

pub(crate) struct SqlStatementSplitter {
    current: String,
    in_single_quote: bool,
    in_double_quote: bool,
    in_backtick: bool,
    escaped: bool,
    in_line_comment: bool,
    in_block_comment: bool,
    pending_dash_comment: bool,
    pending_slash_comment: bool,
    pending_block_comment_end: bool,
}

impl SqlStatementSplitter {
    pub(crate) fn new() -> Self {
        Self {
            current: String::new(),
            in_single_quote: false,
            in_double_quote: false,
            in_backtick: false,
            escaped: false,
            in_line_comment: false,
            in_block_comment: false,
            pending_dash_comment: false,
            pending_slash_comment: false,
            pending_block_comment_end: false,
        }
    }
}

impl SqlImportParser for SqlStatementSplitter {
    fn compactable_insert(&self, statement: &str) -> Option<CompactableInsert> {
        parse_compactable_insert(statement)
    }

    fn push_char(&mut self, ch: char) -> Option<String> {
        if self.in_line_comment {
            if ch == '\n' {
                self.in_line_comment = false;
            }
            return None;
        }

        if self.in_block_comment {
            if self.pending_block_comment_end && ch == '/' {
                self.in_block_comment = false;
                self.pending_block_comment_end = false;
                return None;
            }
            self.pending_block_comment_end = ch == '*';
            return None;
        }

        if self.pending_dash_comment {
            if ch == '-' {
                self.pending_dash_comment = false;
                self.in_line_comment = true;
                return None;
            }
            self.current.push('-');
            self.pending_dash_comment = false;
        }

        if self.pending_slash_comment {
            if ch == '*' {
                self.pending_slash_comment = false;
                self.in_block_comment = true;
                self.pending_block_comment_end = false;
                return None;
            }
            self.current.push('/');
            self.pending_slash_comment = false;
        }

        if self.escaped {
            self.current.push(ch);
            self.escaped = false;
            return None;
        }

        match ch {
            '\\' => {
                self.escaped = true;
                self.current.push(ch);
            }
            '\'' if !self.in_double_quote && !self.in_backtick => {
                self.in_single_quote = !self.in_single_quote;
                self.current.push(ch);
            }
            '"' if !self.in_single_quote && !self.in_backtick => {
                self.in_double_quote = !self.in_double_quote;
                self.current.push(ch);
            }
            '`' if !self.in_single_quote && !self.in_double_quote => {
                self.in_backtick = !self.in_backtick;
                self.current.push(ch);
            }
            '-' if !self.in_single_quote && !self.in_double_quote && !self.in_backtick => {
                self.pending_dash_comment = true;
            }
            '/' if !self.in_single_quote && !self.in_double_quote && !self.in_backtick => {
                self.pending_slash_comment = true;
            }
            ';' if !self.in_single_quote && !self.in_double_quote && !self.in_backtick => {
                let stmt = self.current.trim().to_string();
                self.current.clear();
                if !stmt.is_empty() {
                    return Some(stmt);
                }
            }
            _ => self.current.push(ch),
        }

        None
    }

    fn finish(&mut self) -> Option<String> {
        if self.pending_dash_comment {
            self.current.push('-');
        }
        if self.pending_slash_comment {
            self.current.push('/');
        }
        let stmt = self.current.trim().to_string();
        self.current.clear();
        self.pending_dash_comment = false;
        self.pending_slash_comment = false;
        if stmt.is_empty() {
            None
        } else {
            Some(stmt)
        }
    }
}

fn find_top_level_values_keyword(stmt: &str) -> Option<usize> {
    let bytes = stmt.as_bytes();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_backtick = false;
    let mut escaped = false;
    let mut i = 0usize;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '\'' if !in_double_quote && !in_backtick => in_single_quote = !in_single_quote,
            '"' if !in_single_quote && !in_backtick => in_double_quote = !in_double_quote,
            '`' if !in_single_quote && !in_double_quote => in_backtick = !in_backtick,
            _ => {}
        }

        if !in_single_quote
            && !in_double_quote
            && !in_backtick
            && i + 6 <= bytes.len()
            && bytes[i..i + 6].eq_ignore_ascii_case(b"VALUES")
        {
            let prev_ok =
                i == 0 || !((bytes[i - 1] as char).is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let next_ok = i + 6 == bytes.len()
                || !((bytes[i + 6] as char).is_ascii_alphanumeric() || bytes[i + 6] == b'_');
            if prev_ok && next_ok {
                return Some(i);
            }
        }

        i += 1;
    }

    None
}

pub(crate) fn parse_compactable_insert(stmt: &str) -> Option<CompactableInsert> {
    let values_idx = find_top_level_values_keyword(stmt)?;
    let prefix = stmt[..values_idx].trim_end().to_string();
    if !prefix.to_ascii_uppercase().starts_with("INSERT ") {
        return None;
    }

    let values = stmt[values_idx + "VALUES".len()..].trim().to_string();
    if !values.starts_with('(') || !values.ends_with(')') {
        return None;
    }

    Some(CompactableInsert { prefix, values })
}
