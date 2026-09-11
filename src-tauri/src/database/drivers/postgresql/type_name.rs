//! Validate a type name, not an ALTER clause or arbitrary SQL expression.
//! The server remains responsible for resolving types and checking modifiers.
pub(super) fn validate(input: &str) -> Result<&str, String> {
    let input = input.trim();
    let mut parser = Parser {
        text: input,
        pos: 0,
    };
    let first = parser
        .identifier()
        .ok_or("A PostgreSQL type name is required")?;
    if parser.take('.') {
        parser.identifier().ok_or("Missing qualified type name")?;
        parser.modifiers()?;
    } else {
        match first.to_ascii_lowercase().as_str() {
            "double" => parser.word("precision")?,
            "character" | "char" | "bit" => {
                parser.optional_word("varying");
                parser.modifiers()?;
            }
            "timestamp" | "time" => {
                parser.modifiers()?;
                if parser.optional_word("with") || parser.optional_word("without") {
                    parser.word("time")?;
                    parser.word("zone")?;
                }
            }
            "interval" => {
                parser.modifiers()?;
                let saved = parser.pos;
                if let Some(field) = parser.identifier() {
                    if !["year", "month", "day", "hour", "minute", "second"]
                        .contains(&field.to_ascii_lowercase().as_str())
                    {
                        return Err("Invalid interval field".into());
                    }
                    if parser.optional_word("to") {
                        let end = parser.identifier().ok_or("Missing interval field")?;
                        if !["month", "hour", "minute", "second"]
                            .contains(&end.to_ascii_lowercase().as_str())
                        {
                            return Err("Invalid interval field".into());
                        }
                    }
                    parser.modifiers()?;
                } else {
                    parser.pos = saved;
                }
            }
            _ => parser.modifiers()?,
        }
    }
    while parser.take('[') {
        parser.space();
        if !parser.take(']') {
            parser.integer()?;
            if !parser.take(']') {
                return Err("Unclosed array dimension".into());
            }
        }
    }
    parser.space();
    if parser.pos != input.len() {
        return Err(
            "Enter only a PostgreSQL type; use the SQL editor for custom ALTER clauses".into(),
        );
    }
    Ok(input)
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
}
impl<'a> Parser<'a> {
    fn space(&mut self) {
        while let Some(c) = self.text[self.pos..]
            .chars()
            .next()
            .filter(|c| c.is_whitespace())
        {
            self.pos += c.len_utf8();
        }
    }
    fn take(&mut self, c: char) -> bool {
        self.space();
        if self.text[self.pos..].starts_with(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }
    fn identifier(&mut self) -> Option<&'a str> {
        self.space();
        let start = self.pos;
        if self.take('"') {
            loop {
                let c = self.text[self.pos..].chars().next()?;
                self.pos += c.len_utf8();
                if c == '\0' {
                    return None;
                }
                if c == '"' {
                    if self.text[self.pos..].starts_with('"') {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
            }
        } else {
            let first = self.text[self.pos..].chars().next()?;
            if !(first.is_alphabetic() || first == '_') {
                return None;
            }
            self.pos += first.len_utf8();
            while let Some(c) = self.text[self.pos..]
                .chars()
                .next()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            {
                self.pos += c.len_utf8();
            }
        }
        Some(&self.text[start..self.pos])
    }
    fn optional_word(&mut self, word: &str) -> bool {
        let saved = self.pos;
        if self
            .identifier()
            .is_some_and(|w| w.eq_ignore_ascii_case(word))
        {
            true
        } else {
            self.pos = saved;
            false
        }
    }
    fn word(&mut self, word: &str) -> Result<(), String> {
        if self.optional_word(word) {
            Ok(())
        } else {
            Err(format!("Expected {word} in type name"))
        }
    }
    fn integer(&mut self) -> Result<(), String> {
        self.space();
        self.take('-');
        let start = self.pos;
        while self
            .text
            .as_bytes()
            .get(self.pos)
            .is_some_and(u8::is_ascii_digit)
        {
            self.pos += 1;
        }
        if start == self.pos {
            Err("Expected integer type modifier".into())
        } else {
            Ok(())
        }
    }
    fn modifiers(&mut self) -> Result<(), String> {
        if self.take('(') {
            self.integer()?;
            if self.take(',') {
                self.integer()?;
            }
            if !self.take(')') {
                return Err("Invalid type modifiers".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::validate;
    #[test]
    fn accepts_types_without_accepting_sql_clauses() {
        for ty in [
            "integer",
            "numeric(12,-2)",
            "double precision",
            "character varying(80)",
            "timestamp(3) with time zone",
            "interval day to second(3)",
            "interval",
            "text[][]",
            "\"schema x\".\"Type\"[]",
            "\"a\"\"b\"",
            "public.mood",
            "bit varying(8)",
        ] {
            assert!(validate(ty).is_ok(), "{ty}");
        }
        for ty in [
            "",
            "int; DROP TABLE x",
            "int, DROP COLUMN x",
            "text USING dangerous()",
            "int /*comment*/",
            "int --x",
            "text COLLATE x",
            "text) FROM x",
            "numeric(1,2,3)",
            "\"unterminated",
            "foo.bar.baz",
            "text[]; SELECT 1",
        ] {
            assert!(validate(ty).is_err(), "{ty}");
        }
    }
}
