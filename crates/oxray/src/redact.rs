//! Redacts a capture before it is stored.

use std::collections::BTreeSet;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;

use crate::capture::{BodyCapture, Capture, ErrorEntry, Header, Message, OutgoingCall, Redaction};

const EXACT_KEYS: &[&str] = &["pin", "cvv", "cvc", "cvv2", "otp", "pan"];
const CONTAINS_KEYS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "apikey",
    "privatekey",
    "cardnumber",
    "authorization",
];
const DEFAULT_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
];

const MASKED: &str = "[redacted]";

/// Rules used to redact one capture.
#[derive(Clone, Debug)]
pub struct Redactor {
    headers: BTreeSet<String>,
    extra_keys: BTreeSet<String>,
}

impl Redactor {
    /// Default header and key rules, plus caller extras.
    pub fn new(extra_headers: &[String], extra_keys: &[String]) -> Self {
        let mut headers = BTreeSet::new();
        for name in DEFAULT_HEADERS {
            headers.insert((*name).to_owned());
        }
        for name in extra_headers {
            headers.insert(name.to_ascii_lowercase());
        }
        let mut keys = BTreeSet::new();
        for key in extra_keys {
            keys.insert(normalize_key(key));
        }
        Self {
            headers,
            extra_keys: keys,
        }
    }

    /// Rule names reported by `/api/meta`.
    pub fn rule_names(&self) -> &'static [&'static str] {
        &[
            "sensitive_header",
            "sensitive_key",
            "clabe",
            "pan",
            "truncated_digits",
        ]
    }

    /// Redacts `capture` in place and records each hit.
    pub fn apply(&self, capture: &mut Capture) {
        let mut redactions = Vec::new();
        capture.path = mask_secrets(&capture.path, "/path", &mut redactions);
        capture.query = self.redact_query(&capture.query, &mut redactions);
        self.redact_headers(
            &mut capture.request.headers,
            "/request/headers",
            &mut redactions,
        );
        self.redact_body(&mut capture.request.body, "/request/body", &mut redactions);
        if let Some(response) = capture.response.as_mut() {
            self.redact_message(response, "/response", &mut redactions);
        }
        for (index, log) in capture.logs.iter_mut().enumerate() {
            let path = format!("/logs/{index}/message");
            log.message = mask_secrets(&log.message, &path, &mut redactions);
            let keys: Vec<String> = log.fields.keys().cloned().collect();
            for key in keys {
                let Some(value) = log.fields.get_mut(&key) else {
                    continue;
                };
                if self.sensitive_key(&normalize_key(&key)) {
                    *value = Value::String(MASKED.to_owned());
                    redactions.push(Redaction {
                        path: format!("/logs/{index}/fields/{}", escape_pointer(&key)),
                        rule: "sensitive_key".to_owned(),
                    });
                } else {
                    walk_json(
                        value,
                        &format!("/logs/{index}/fields/{}", escape_pointer(&key)),
                        self,
                        &mut redactions,
                    );
                }
            }
        }
        for (index, query) in capture.queries.iter_mut().enumerate() {
            query.sql = mask_secrets(
                &query.sql,
                &format!("/queries/{index}/sql"),
                &mut redactions,
            );
        }
        for (index, call) in capture.outgoing.iter_mut().enumerate() {
            self.redact_outgoing(call, index, &mut redactions);
        }
        for (index, error) in capture.errors.iter_mut().enumerate() {
            self.redact_error(error, index, &mut redactions);
        }
        for (index, group) in capture.n_plus_one.iter_mut().enumerate() {
            let prefix = format!("/n_plus_one/{index}");
            group.sql = mask_secrets(&group.sql, &format!("{prefix}/sql"), &mut redactions);
            group.fingerprint = mask_secrets(
                &group.fingerprint,
                &format!("{prefix}/fingerprint"),
                &mut redactions,
            );
        }
        capture.redactions = redactions;
    }

    fn redact_message(&self, message: &mut Message, prefix: &str, redactions: &mut Vec<Redaction>) {
        self.redact_headers(
            &mut message.headers,
            &format!("{prefix}/headers"),
            redactions,
        );
        self.redact_body(&mut message.body, &format!("{prefix}/body"), redactions);
    }

    fn redact_outgoing(
        &self,
        call: &mut OutgoingCall,
        index: usize,
        redactions: &mut Vec<Redaction>,
    ) {
        let prefix = format!("/outgoing/{index}");
        call.url = mask_secrets(&call.url, &format!("{prefix}/url"), redactions);
        self.redact_message(&mut call.request, &format!("{prefix}/request"), redactions);
        if let Some(response) = call.response.as_mut() {
            self.redact_message(response, &format!("{prefix}/response"), redactions);
        }
        if let Some(error) = call.error.as_mut() {
            *error = mask_secrets(error, &format!("{prefix}/error"), redactions);
        }
    }

    fn redact_error(&self, error: &mut ErrorEntry, index: usize, redactions: &mut Vec<Redaction>) {
        let prefix = format!("/errors/{index}");
        error.message = mask_secrets(&error.message, &format!("{prefix}/message"), redactions);
        if let Some(target) = error.target.as_mut() {
            *target = mask_secrets(target, &format!("{prefix}/target"), redactions);
        }
        if let Some(location) = error.location.as_mut() {
            *location = mask_secrets(location, &format!("{prefix}/location"), redactions);
        }
        for (frame_index, frame) in error.backtrace.iter_mut().enumerate() {
            let frame_prefix = format!("{prefix}/backtrace/{frame_index}");
            frame.function = mask_secrets(
                &frame.function,
                &format!("{frame_prefix}/function"),
                redactions,
            );
            if let Some(file) = frame.file.as_mut() {
                *file = mask_secrets(file, &format!("{frame_prefix}/file"), redactions);
            }
        }
    }

    fn redact_headers(
        &self,
        headers: &mut [Header],
        prefix: &str,
        redactions: &mut Vec<Redaction>,
    ) {
        for header in headers.iter_mut() {
            let name = header.name.to_ascii_lowercase();
            let path = format!("{prefix}/{}", escape_pointer(&name));
            if self.headers.contains(&name) {
                header.value = MASKED.to_owned();
                redactions.push(Redaction {
                    path,
                    rule: "sensitive_header".to_owned(),
                });
            } else {
                header.value = mask_secrets(&header.value, &path, redactions);
            }
        }
    }

    fn redact_query(&self, query: &str, redactions: &mut Vec<Redaction>) -> String {
        if query.is_empty() {
            return String::new();
        }
        let mut parts = Vec::new();
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = percent_decode(key);
            let value = percent_decode(value);
            let path = format!("/request/query/{}", escape_pointer(&key));
            let value = if self.sensitive_key(&normalize_key(&key)) {
                redactions.push(Redaction {
                    path,
                    rule: "sensitive_key".to_owned(),
                });
                MASKED.to_owned()
            } else {
                mask_secrets(&value, &path, redactions)
            };
            parts.push(format!(
                "{}={}",
                utf8_percent_encode(&key, NON_ALPHANUMERIC),
                utf8_percent_encode(&value, NON_ALPHANUMERIC)
            ));
        }
        parts.join("&")
    }

    fn redact_body(&self, body: &mut BodyCapture, prefix: &str, redactions: &mut Vec<Redaction>) {
        let Some(text) = body.text.clone() else {
            return;
        };
        let content_type = body
            .content_type
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        let redacted = if is_json(&content_type) {
            redact_json_text(&text, prefix, self, redactions)
        } else if is_form(&content_type) {
            self.redact_form(&text, prefix, redactions)
        } else {
            mask_secrets(&text, prefix, redactions)
        };
        let redacted = if body.truncated {
            mask_edge_digits(&redacted, prefix, redactions)
        } else {
            redacted
        };
        body.text = Some(redacted);
    }

    fn redact_form(&self, text: &str, prefix: &str, redactions: &mut Vec<Redaction>) -> String {
        let mut parts = Vec::new();
        for pair in text.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = percent_decode(key);
            let value = percent_decode(value);
            let path = format!("{prefix}/{}", escape_pointer(&key));
            let value = if self.sensitive_key(&normalize_key(&key)) {
                redactions.push(Redaction {
                    path,
                    rule: "sensitive_key".to_owned(),
                });
                MASKED.to_owned()
            } else {
                mask_secrets(&value, &path, redactions)
            };
            parts.push(format!(
                "{}={}",
                utf8_percent_encode(&key, FORM_SET),
                utf8_percent_encode(&value, FORM_SET)
            ));
        }
        parts.join("&")
    }

    fn sensitive_key(&self, normalized: &str) -> bool {
        if self.extra_keys.contains(normalized) || EXACT_KEYS.contains(&normalized) {
            return true;
        }
        CONTAINS_KEYS
            .iter()
            .any(|needle| normalized.contains(needle))
    }
}

const FORM_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'*')
    .remove(b'-')
    .remove(b'.')
    .remove(b'_');

fn redact_json_text(
    text: &str,
    prefix: &str,
    redactor: &Redactor,
    redactions: &mut Vec<Redaction>,
) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(mut value) => {
            walk_json(&mut value, prefix, redactor, redactions);
            serde_json::to_string(&value)
                .unwrap_or_else(|_| tolerant_json(text, prefix, redactor, redactions))
        }
        Err(_) => tolerant_json(text, prefix, redactor, redactions),
    }
}

fn walk_json(
    value: &mut Value,
    pointer: &str,
    redactor: &Redactor,
    redactions: &mut Vec<Redaction>,
) {
    if let Some(text) = json_number(value) {
        let masked = mask_secrets(&text, pointer, redactions);
        if masked != text {
            *value = Value::String(masked);
        }
        return;
    }
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let path = format!("{pointer}/{}", escape_pointer(&key));
                let Some(child) = map.get_mut(&key) else {
                    continue;
                };
                if redactor.sensitive_key(&normalize_key(&key)) {
                    *child = Value::String(MASKED.to_owned());
                    redactions.push(Redaction {
                        path,
                        rule: "sensitive_key".to_owned(),
                    });
                } else {
                    walk_json(child, &path, redactor, redactions);
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                walk_json(item, &format!("{pointer}/{index}"), redactor, redactions);
            }
        }
        Value::String(text) => {
            *text = mask_secrets(text, pointer, redactions);
        }
        _ => {}
    }
}

fn json_number(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn tolerant_json(
    text: &str,
    prefix: &str,
    redactor: &Redactor,
    redactions: &mut Vec<Redaction>,
) -> String {
    let mut output = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && let Some(key_end) = closing_quote(text, index)
        {
            let key = &text[index + 1..key_end];
            let after = skip_ws(text, key_end + 1);
            if after < text.len()
                && text.as_bytes()[after] == b':'
                && redactor.sensitive_key(&normalize_key(key))
            {
                output.push_str(&text[index..=key_end]);
                output.push(':');
                output.push_str(&format!("\"{MASKED}\""));
                redactions.push(Redaction {
                    path: format!("{prefix}/{}", escape_pointer(key)),
                    rule: "sensitive_key".to_owned(),
                });
                index = skip_value(text, skip_ws(text, after + 1));
                continue;
            }
        }
        output.push(text[index..].chars().next().unwrap_or(' '));
        index += text[index..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
    }
    mask_secrets(&output, prefix, redactions)
}

fn closing_quote(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = open + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn skip_ws(text: &str, mut index: usize) -> usize {
    while index < text.len() && text.as_bytes()[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn skip_value(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let bytes = text.as_bytes();
    match bytes[index] {
        b'"' => closing_quote(text, index)
            .map(|end| end + 1)
            .unwrap_or(text.len()),
        b'{' | b'[' => skip_container(text, index),
        _ => {
            let mut end = index;
            while end < bytes.len() && !matches!(bytes[end], b',' | b'}' | b']') {
                end += 1;
            }
            end
        }
    }
}

fn skip_container(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut index = start;
    let mut in_string = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
    text.len()
}

fn mask_secrets(text: &str, path: &str, redactions: &mut Vec<Redaction>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    let mut changed = false;
    while index < chars.len() {
        if chars[index].is_ascii_digit() {
            let (end, digits) = read_number(&chars, index);
            if let Some(rule) = classify(&digits) {
                output.push_str(&mask_digits(&digits));
                changed = true;
                index = end;
                if !redactions
                    .iter()
                    .any(|item| item.path == path && item.rule == rule)
                {
                    redactions.push(Redaction {
                        path: path.to_owned(),
                        rule: rule.to_owned(),
                    });
                }
                continue;
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    if changed { output } else { text.to_owned() }
}

fn mask_edge_digits(text: &str, path: &str, redactions: &mut Vec<Redaction>) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() || !chars[chars.len() - 1].is_ascii_digit() {
        return text.to_owned();
    }
    let mut start = chars.len() - 1;
    while start > 0 {
        let previous = chars[start - 1];
        if previous.is_ascii_digit() || previous == ' ' || previous == '-' {
            start -= 1;
        } else {
            break;
        }
    }
    while start < chars.len() && !chars[start].is_ascii_digit() {
        start += 1;
    }
    if start >= chars.len() {
        return text.to_owned();
    }
    let mut output: String = chars[..start].iter().collect();
    output.push_str(MASKED);
    redactions.push(Redaction {
        path: path.to_owned(),
        rule: "truncated_digits".to_owned(),
    });
    output
}

fn read_number(chars: &[char], start: usize) -> (usize, String) {
    let mut digits = String::new();
    let mut index = start;
    while index < chars.len() {
        if chars[index].is_ascii_digit() {
            digits.push(chars[index]);
            index += 1;
            continue;
        }
        if (chars[index] == ' ' || chars[index] == '-')
            && index + 1 < chars.len()
            && chars[index + 1].is_ascii_digit()
        {
            index += 1;
            continue;
        }
        break;
    }
    (index, digits)
}

fn classify(digits: &str) -> Option<&'static str> {
    if digits.len() == 18 && clabe_ok(digits) {
        return Some("clabe");
    }
    if (13..=19).contains(&digits.len()) && luhn_ok(digits) {
        return Some("pan");
    }
    None
}

fn mask_digits(digits: &str) -> String {
    let last = digits.chars().rev().take(4).collect::<String>();
    let last: String = last.chars().rev().collect();
    format!("•••• {last}")
}

fn clabe_ok(digits: &str) -> bool {
    let bytes = digits.as_bytes();
    if bytes.len() != 18 || !bytes.iter().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let weights = [3_u32, 7, 1];
    let mut sum = 0_u32;
    for (index, byte) in bytes[..17].iter().enumerate() {
        let digit = u32::from(byte - b'0');
        sum += (digit * weights[index % 3]) % 10;
    }
    let expected = (10 - (sum % 10)) % 10;
    u32::from(bytes[17] - b'0') == expected
}

fn luhn_ok(digits: &str) -> bool {
    let mut sum = 0_u32;
    let mut double = false;
    for byte in digits.bytes().rev() {
        if !byte.is_ascii_digit() {
            return false;
        }
        let mut digit = u32::from(byte - b'0');
        if double {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
        double = !double;
    }
    sum.is_multiple_of(10)
}

pub(crate) fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|ch| *ch != '_' && *ch != '-' && *ch != ' ')
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn percent_decode(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .into_owned()
}

fn is_json(content_type: &str) -> bool {
    content_type.contains("application/json") || content_type.contains("+json")
}

fn is_form(content_type: &str) -> bool {
    content_type.contains("application/x-www-form-urlencoded")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::Capture;
    use proptest::prop_assert;

    #[test]
    fn known_pan_and_clabe() {
        assert!(luhn_ok("4242424242424242"));
        assert!(clabe_ok("032180000118359719"));
        assert!(!luhn_ok("4242424242424243"));
    }

    proptest::proptest! {
        #[test]
        fn random_wrappers_do_not_keep_a_pan(prefix in "[a-z]{0,8}", suffix in "[a-z]{0,8}") {
            let pan = "4242424242424242";
            let mut capture: Capture = serde_json::from_str(include_str!("../tests/fixtures/capture_v1.json")).unwrap();
            capture.request.body.content_type = Some("text/plain".to_owned());
            capture.request.body.text = Some(format!("{prefix} {pan} {suffix}"));
            capture.request.body.truncated = false;
            Redactor::new(&[], &[]).apply(&mut capture);
            let text = capture.request.body.text.unwrap_or_default();
            prop_assert!(!text.contains(pan));
        }
    }
}
