//! SQL fingerprint used to group repeated queries.

/// Normalized SQL: literals and parameters become `?`, whitespace collapses,
/// and text outside quotes is lowercase.
pub(crate) fn fingerprint(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut quoting = false;
    while let Some(ch) = chars.next() {
        if quoting {
            if ch == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    continue;
                }
                quoting = false;
                push_mark(&mut out);
            }
            continue;
        }
        if ch == '-' && chars.peek() == Some(&'-') {
            chars.next();
            while chars.peek().is_some_and(|next| *next != '\n') {
                chars.next();
            }
            push_space(&mut out);
            continue;
        }
        if ch == '\'' {
            quoting = true;
            continue;
        }
        if ch == '$' && chars.peek().is_some_and(|next| next.is_ascii_digit()) {
            while chars.peek().is_some_and(|next| next.is_ascii_digit()) {
                chars.next();
            }
            push_mark(&mut out);
            continue;
        }
        if ch == '?' {
            push_mark(&mut out);
            continue;
        }
        if ch.is_ascii_digit() {
            if continues_identifier(&out) {
                push_identifier_char(&mut out, ch);
                while chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
                {
                    if let Some(next) = chars.next() {
                        push_identifier_char(&mut out, next);
                    }
                }
                continue;
            }
            while chars
                .peek()
                .is_some_and(|next| next.is_ascii_digit() || *next == '.')
            {
                chars.next();
            }
            push_mark(&mut out);
            continue;
        }
        if ch.is_whitespace() {
            push_space(&mut out);
            continue;
        }
        for lower in ch.to_lowercase() {
            out.push(lower);
        }
    }
    if quoting {
        push_mark(&mut out);
    }
    out.trim().to_owned()
}

fn push_identifier_char(out: &mut String, ch: char) {
    if ch.is_ascii_digit() || ch == '_' {
        out.push(ch);
        return;
    }
    for lower in ch.to_lowercase() {
        out.push(lower);
    }
}

fn continues_identifier(out: &str) -> bool {
    out.ends_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
}

fn push_mark(out: &mut String) {
    if !out.ends_with('?') {
        out.push('?');
    }
}

fn push_space(out: &mut String) {
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::fingerprint;

    #[test]
    fn literals_and_case_collapse() {
        assert_eq!(
            fingerprint("SELECT id FROM payments WHERE pan = '4242' AND id = 14"),
            "select id from payments where pan = ? and id = ?"
        );
        assert_eq!(
            fingerprint("select  id\nfrom t where n = $1"),
            "select id from t where n = ?"
        );
    }

    #[test]
    fn digits_inside_identifiers_stay_distinct() {
        assert_eq!(fingerprint("select * from table1"), "select * from table1");
        assert_eq!(fingerprint("select * from table2"), "select * from table2");
        assert_ne!(
            fingerprint("select * from table1 where id = 1"),
            fingerprint("select * from table2 where id = 1")
        );
        assert_eq!(
            fingerprint("select * from table1 where id = 1"),
            "select * from table1 where id = ?"
        );
        assert_eq!(
            fingerprint("SELECT * FROM Table1Name WHERE id = 1"),
            fingerprint("select * from table1name where id = 1")
        );
        assert_ne!(
            fingerprint("select * from table1"),
            fingerprint("select * from table2")
        );
    }
}
