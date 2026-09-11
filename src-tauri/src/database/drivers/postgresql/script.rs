//! PostgreSQL lexical splitting. COPY payloads are read by the runner, never
//! interpreted as SQL. Plain SQL is UTF8 with standard_conforming_strings=on.

pub(super) const MAX_STATEMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, PartialEq)]
enum State {
    Normal,
    Single(bool),
    Double,
    Dollar(Vec<u8>),
    LineComment,
    BlockComment(usize),
}

#[derive(Debug)]
pub(super) struct Statement {
    pub sql: String,
    pub words: Vec<String>,
    pub line: usize,
}

impl Statement {
    pub fn is_copy(&self) -> bool {
        self.words.first().is_some_and(|w| w == "COPY")
    }

    pub fn validate_copy(&self) -> Result<(), String> {
        if !self.words.windows(2).any(|w| w == ["FROM", "STDIN"]) {
            return Err("Only COPY ... FROM STDIN is supported; server files, PROGRAM and COPY TO are not imported".into());
        }
        Ok(())
    }

    pub fn transaction_wrapper(&self) -> Result<bool, String> {
        let words = self.words.iter().map(String::as_str).collect::<Vec<_>>();
        match words.as_slice() {
            ["BEGIN" | "COMMIT" | "END"] |
            ["BEGIN" | "COMMIT" | "END", "WORK" | "TRANSACTION"] |
            ["START", "TRANSACTION"] => Ok(true),
            ["BEGIN" | "COMMIT" | "END" | "ROLLBACK" | "ABORT", ..] |
            ["START", "TRANSACTION", ..] | ["PREPARE", "TRANSACTION", ..] =>
                Err("Import uses one transaction. Only plain BEGIN/COMMIT wrappers are supported; remove custom transaction control".into()),
            _ => Ok(false),
        }
    }
}

pub(super) struct Parser {
    state: State,
    current: Vec<u8>,
    word: Vec<u8>,
    words: Vec<String>,
    depth: usize,
    has_sql: bool,
    start_line: usize,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Normal,
            current: Vec::new(),
            word: Vec::new(),
            words: Vec::new(),
            depth: 0,
            has_sql: false,
            start_line: 1,
        }
    }

    pub fn is_idle(&self) -> bool {
        !self.has_sql && self.state == State::Normal
    }

    fn flush_word(&mut self) {
        if !self.word.is_empty() {
            if self.depth == 0 {
                self.words
                    .push(String::from_utf8_lossy(&self.word).to_ascii_uppercase());
            }
            self.word.clear();
        }
    }

    fn take(&mut self) -> Option<Statement> {
        self.flush_word();
        let bytes = std::mem::take(&mut self.current);
        let words = std::mem::take(&mut self.words);
        if !std::mem::take(&mut self.has_sql) {
            return None;
        }
        Some(Statement {
            sql: String::from_utf8(bytes).expect("input lines are UTF8"),
            words,
            line: self.start_line,
        })
    }

    /// Consume up to one statement, allowing execution before parsing further
    /// input. The byte offset is always left at a UTF8 boundary when returning.
    pub fn consume(
        &mut self,
        line: &str,
        offset: &mut usize,
        line_number: usize,
    ) -> Result<Option<Statement>, String> {
        let bytes = line.as_bytes();
        while *offset < bytes.len() {
            if self.current.len() >= MAX_STATEMENT_BYTES {
                return Err(
                    "SQL statement exceeds the 16 MiB import limit; prefer a COPY-based dump"
                        .into(),
                );
            }
            let i = *offset;
            let b = bytes[i];
            if b == 0 {
                return Err(
                    "NUL byte in SQL input; use a UTF8 plain SQL dump, not a binary archive".into(),
                );
            }
            let next = bytes.get(i + 1).copied();
            let mut width = 1;
            match &mut self.state {
                State::LineComment => {
                    if b == b'\n' {
                        self.state = State::Normal;
                    }
                }
                State::BlockComment(depth) => {
                    if b == b'/' && next == Some(b'*') {
                        *depth += 1;
                        width = 2;
                    } else if b == b'*' && next == Some(b'/') {
                        *depth -= 1;
                        width = 2;
                        if *depth == 0 {
                            self.state = State::Normal;
                        }
                    }
                }
                State::Single(escape) => {
                    if *escape && b == b'\\' && next.is_some() {
                        width = 2;
                    } else if b == b'\'' {
                        if next == Some(b'\'') {
                            width = 2;
                        } else {
                            self.state = State::Normal;
                        }
                    }
                }
                State::Double => {
                    if b == b'"' {
                        if next == Some(b'"') {
                            width = 2;
                        } else {
                            self.state = State::Normal;
                        }
                    }
                }
                State::Dollar(tag) => {
                    if bytes[i..].starts_with(tag) {
                        width = tag.len();
                        self.state = State::Normal;
                    }
                }
                State::Normal => {
                    if b == b'-' && next == Some(b'-') {
                        self.flush_word();
                        self.state = State::LineComment;
                        width = 2;
                    } else if b == b'/' && next == Some(b'*') {
                        self.flush_word();
                        self.state = State::BlockComment(1);
                        width = 2;
                    } else if b == b';' {
                        if self.depth != 0 {
                            return Err("Unbalanced SQL parentheses before semicolon".into());
                        }
                        *offset += 1;
                        if let Some(statement) = self.take() {
                            return Ok(Some(statement));
                        }
                        continue;
                    } else {
                        if !b.is_ascii_whitespace() && !self.has_sql {
                            self.start_line = line_number;
                            self.has_sql = true;
                        }
                        if b == b'\'' {
                            let escape = self.word.eq_ignore_ascii_case(b"e");
                            self.flush_word();
                            self.state = State::Single(escape);
                            if self.depth == 0 {
                                self.words.push("<literal>".into());
                            }
                        } else if b == b'"' {
                            self.flush_word();
                            self.state = State::Double;
                            if self.depth == 0 {
                                self.words.push("<identifier>".into());
                            }
                        } else if b == b'$' && self.word.is_empty() {
                            if let Some(length) = dollar_tag_length(&bytes[i..]) {
                                width = length;
                                self.state = State::Dollar(bytes[i..i + length].to_vec());
                                if self.depth == 0 {
                                    self.words.push("<literal>".into());
                                }
                            } else {
                                self.flush_word();
                            }
                        } else if b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 128 {
                            self.word.push(b);
                        } else {
                            self.flush_word();
                            if b == b'(' {
                                if self.depth == 0 {
                                    self.words.push("<group>".into());
                                }
                                self.depth += 1;
                            } else if b == b')' {
                                self.depth = self
                                    .depth
                                    .checked_sub(1)
                                    .ok_or("Unbalanced SQL parentheses")?;
                            } else if !b.is_ascii_whitespace() && self.depth == 0 {
                                self.words.push("<symbol>".into());
                            }
                            if b == b'\\' {
                                return Err("psql commands are not SQL; use a plain single-database dump without \\connect, \\i or shell commands".into());
                            }
                        }
                    }
                }
            }
            self.current.extend_from_slice(&bytes[i..i + width]);
            *offset += width;
        }
        Ok(None)
    }

    pub fn finish(&mut self) -> Result<Option<Statement>, String> {
        if !matches!(self.state, State::Normal | State::LineComment) {
            return Err(format!(
                "Unterminated SQL quote, dollar block or comment starting near line {}",
                self.start_line
            ));
        }
        if self.depth != 0 {
            return Err("Unbalanced SQL parentheses at end of file".into());
        }
        Ok(self.take())
    }
}

fn dollar_tag_length(bytes: &[u8]) -> Option<usize> {
    if bytes.get(1) == Some(&b'$') {
        return Some(2);
    }
    let first = *bytes.get(1)?;
    if !(first.is_ascii_alphabetic() || first == b'_' || first >= 128) {
        return None;
    }
    for (i, b) in bytes.iter().copied().enumerate().skip(2) {
        if b == b'$' {
            return Some(i + 1);
        }
        if !(b.is_ascii_alphanumeric() || b == b'_' || b >= 128) {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn split(sql: &str) -> Result<Vec<Statement>, String> {
        let mut parser = Parser::new();
        let mut statements = Vec::new();
        for (n, line) in sql.split_inclusive('\n').enumerate() {
            let mut offset = 0;
            while let Some(statement) = parser.consume(line, &mut offset, n + 1)? {
                statements.push(statement);
            }
        }
        if let Some(statement) = parser.finish()? {
            statements.push(statement);
        }
        Ok(statements)
    }

    #[test]
    fn keeps_dollar_functions_nested_comments_and_quoted_identifiers() {
        let sql = "/* outer /* inner ; */ still comment */ CREATE FUNCTION f() RETURNS text AS $body$ BEGIN RETURN $$a;b$$; END; $body$ LANGUAGE plpgsql;\nSELECT \"a;\"\"b\", 'O''Reilly; 🐘'; -- trailing ;\nSELECT 3";
        let statements = split(sql).unwrap();
        assert_eq!(statements.len(), 3);
        assert!(statements[0].sql.contains("RETURN $$a;b$$;"));
        assert_eq!(statements[1].line, 2);
        assert_eq!(statements[2].words, vec!["SELECT", "3"]);
    }

    #[test]
    fn distinguishes_escape_strings_from_standard_backslashes_and_dollar_identifiers() {
        let statements =
            split(r"SELECT E'it\'s;ok', 'C:\'; SELECT foo$bar FROM t; SELECT $1;").unwrap();
        assert_eq!(statements.len(), 3);
        assert!(statements[1].sql.contains("foo$bar"));
    }

    #[test]
    fn rejects_incomplete_input_and_client_commands() {
        for sql in [
            "SELECT 'oops",
            "DO $tag$ BEGIN;",
            "/* unclosed",
            "SELECT (1",
            "\\connect other\n",
            "SELECT '\0'",
        ] {
            assert!(split(sql).is_err(), "{sql}");
        }
    }

    #[test]
    fn identifies_copy_and_guards_transaction_control() {
        let statements = split("COPY public.t (id, value) FROM stdin; BEGIN; COMMIT; ROLLBACK; PREPARE TRANSACTION 'x';").unwrap();
        assert!(statements[0].is_copy());
        assert!(statements[0].validate_copy().is_ok());
        assert!(statements[1].transaction_wrapper().unwrap());
        assert!(statements[2].transaction_wrapper().unwrap());
        assert!(statements[3].transaction_wrapper().is_err());
        assert!(statements[4].transaction_wrapper().is_err());
        for sql in [
            "COPY t FROM PROGRAM 'echo unsafe';",
            "COPY t TO STDOUT;",
            "COPY (SELECT 'FROM STDIN') TO STDOUT;",
        ] {
            assert!(split(sql).unwrap()[0].validate_copy().is_err());
        }
    }
}
