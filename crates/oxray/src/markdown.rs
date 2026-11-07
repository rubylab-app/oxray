//! Markdown rendering for the "Copy as Markdown" contract.

use std::time::Duration;

use crate::capture::{BodyCapture, BodyEncoding, Capture, ErrorKind, Message, QueryEntry};
use crate::format_duration;

/// Maximum log lines rendered before the `…and n more` line.
const MAX_LOG_LINES: usize = 50;

impl Capture {
    /// Renders this capture as a Markdown document for coding agents.
    ///
    /// The document follows the "Copy as Markdown" contract: a header with the
    /// method, matched path, status, and duration, followed by the request,
    /// response, queries, outgoing calls, errors, and logs sections. Sections
    /// without content are omitted, except the request. The document always
    /// ends with exactly one trailing newline.
    pub fn markdown(&self) -> String {
        render(self)
    }
}

fn render(capture: &Capture) -> String {
    let mut doc = Doc::default();
    let title_path = capture
        .matched_path
        .as_deref()
        .unwrap_or(capture.path.as_str());
    let status = match capture.status {
        Some(code) => code.to_string(),
        None => "no response".to_owned(),
    };
    doc.push(format!(
        "## {} {title_path} → {status} in {}",
        capture.method,
        format_duration(Duration::from_micros(capture.duration_us)),
    ));
    doc.push(format!(
        "- Captured: {} · id {}",
        capture.started_at, capture.id
    ));
    match capture.matched_path.as_deref() {
        None => doc.push(format!("- Route: none (path {})", capture.path)),
        Some(matched) if matched != capture.path => {
            doc.push(format!("- Route: {matched} (path {})", capture.path));
        }
        Some(_) => {}
    }
    if !capture.redactions.is_empty() {
        doc.push(format!(
            "- Redacted: {}",
            counted(capture.redactions.len(), "value")
        ));
    }

    doc.blank();
    doc.push("### Request");
    render_message(&mut doc, &capture.request);

    doc.blank();
    doc.push("### Response");
    match &capture.response {
        Some(message) => render_message(&mut doc, message),
        None => doc.push("No response: the handler panicked before responding."),
    }

    if !capture.queries.is_empty() {
        doc.blank();
        render_queries(&mut doc, capture);
    }
    if !capture.outgoing.is_empty() {
        doc.blank();
        doc.push(format!("### Outgoing ({})", capture.outgoing.len()));
        for (index, call) in capture.outgoing.iter().enumerate() {
            let outcome = match call.status {
                Some(code) => code.to_string(),
                None => match &call.error {
                    Some(error) => format!("error: {error}"),
                    None => "no response".to_owned(),
                },
            };
            doc.push(format!(
                "{}. {} {} → {outcome} in {}",
                index + 1,
                call.method,
                call.url,
                format_duration(Duration::from_micros(call.duration_us)),
            ));
        }
    }
    if !capture.errors.is_empty() {
        doc.blank();
        doc.push(format!("### Errors ({})", capture.errors.len()));
        for error in &capture.errors {
            // Compared by value so future `ErrorKind` variants fall through
            // to the plain error line even though the enum is `non_exhaustive`.
            let line = if error.kind == ErrorKind::Panic {
                match &error.location {
                    Some(location) => format!("- panic at {location}: {}", error.message),
                    None => format!("- panic: {}", error.message),
                }
            } else if error.kind == ErrorKind::ErrorEvent {
                match &error.target {
                    Some(target) => format!("- error {target}: {}", error.message),
                    None => format!("- error: {}", error.message),
                }
            } else {
                format!("- error: {}", error.message)
            };
            doc.push(line);
        }
    }
    if !capture.logs.is_empty() {
        doc.blank();
        doc.push(format!("### Logs ({})", capture.logs.len()));
        let shown = capture.logs.len().min(MAX_LOG_LINES);
        for log in &capture.logs[..shown] {
            doc.push(format!("- {} {}: {}", log.level, log.target, log.message));
        }
        if capture.logs.len() > MAX_LOG_LINES {
            doc.push(format!("…and {} more", capture.logs.len() - MAX_LOG_LINES));
        }
    }
    doc.finish()
}

fn render_message(doc: &mut Doc, message: &Message) {
    let has_headers = !message.headers.is_empty();
    if has_headers {
        let mut block = String::new();
        for header in &message.headers {
            if !block.is_empty() {
                block.push('\n');
            }
            block.push_str(&format!(
                "{}: {}",
                header.name.to_ascii_lowercase(),
                header.value
            ));
        }
        doc.push(fence("http", &block));
    }
    let body = body_lines(&message.body);
    if body.iter().any(|line| !line.is_empty()) {
        if has_headers && body[0].starts_with("```") {
            doc.blank();
        }
        for line in body {
            doc.push(line);
        }
    }
}

/// Body content lines: fenced blocks, notice lines, and the blank line that
/// separates a truncated body block from its truncation notice.
fn body_lines(body: &BodyCapture) -> Vec<String> {
    let mut lines = Vec::new();
    if body.truncated {
        if let Some(text) = &body.text
            && !text.is_empty()
        {
            lines.push(fence("text", text));
            if let Some(notice) = body_notice(body) {
                lines.push(String::new());
                lines.push(notice);
            }
        } else if let Some(notice) = body_notice(body) {
            lines.push(notice);
        }
        return lines;
    }
    if body.encoding == BodyEncoding::Omitted || body.omitted_reason.is_some() {
        if let Some(notice) = body_notice(body) {
            lines.push(notice);
        }
        return lines;
    }
    let Some(text) = &body.text else {
        return lines;
    };
    if text.is_empty() {
        return lines;
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => match serde_json::to_string_pretty(&value) {
            Ok(pretty) => lines.push(fence("json", &pretty)),
            Err(_) => lines.push(fence("text", text)),
        },
        Err(_) => lines.push(fence("text", text)),
    }
    lines
}

fn render_queries(doc: &mut Doc, capture: &Capture) {
    let mut groups: Vec<(&str, Vec<&QueryEntry>)> = Vec::new();
    for query in &capture.queries {
        match groups
            .iter_mut()
            .find(|(fingerprint, _)| *fingerprint == query.fingerprint)
        {
            Some((_, entries)) => entries.push(query),
            None => groups.push((query.fingerprint.as_str(), vec![query])),
        }
    }
    let n_plus_one_groups = groups
        .iter()
        .filter(|(fingerprint, _)| {
            capture
                .n_plus_one
                .iter()
                .any(|group| group.fingerprint == *fingerprint)
        })
        .count();
    let mut heading = format!(
        "### Queries ({}, {} distinct",
        counted(capture.queries.len(), "execution"),
        groups.len()
    );
    if n_plus_one_groups > 0 {
        heading.push_str(&format!(
            ", {} N+1 {}",
            n_plus_one_groups,
            if n_plus_one_groups == 1 {
                "group"
            } else {
                "groups"
            }
        ));
    }
    heading.push(')');
    doc.push(heading);
    for (index, (fingerprint, entries)) in groups.iter().enumerate() {
        let sql = inline_code(entries[0].sql.as_str());
        let marker = if capture
            .n_plus_one
            .iter()
            .any(|group| group.fingerprint == *fingerprint)
        {
            " · **N+1**"
        } else {
            ""
        };
        if entries.len() == 1 {
            let query = entries[0];
            let mut line = format!(
                "{}. {sql} · {}",
                index + 1,
                format_duration(Duration::from_micros(query.duration_us))
            );
            if let Some(rows) = query.rows_returned.or(query.rows_affected) {
                line.push_str(&format!(
                    " · {}",
                    counted(usize::try_from(rows).unwrap_or(usize::MAX), "row")
                ));
            }
            line.push_str(marker);
            doc.push(line);
        } else {
            let total_us: u128 = entries
                .iter()
                .map(|query| u128::from(query.duration_us))
                .sum();
            let total = u64::try_from(total_us).unwrap_or(u64::MAX);
            doc.push(format!(
                "{}. {sql} · ×{} · {} total{marker}",
                index + 1,
                entries.len(),
                format_duration(Duration::from_micros(total)),
            ));
        }
    }
}

/// `1 value` / `n values`.
fn counted(n: usize, singular: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {singular}s")
    }
}

/// Wraps `sql` in backticks, doubling them with spaces when it contains one.
fn inline_code(sql: &str) -> String {
    if sql.contains('`') {
        format!("`` {sql} ``")
    } else {
        format!("`{sql}`")
    }
}

/// Fenced block whose delimiter is one backtick longer than the longest run
/// of backticks in `content`, and never shorter than three.
fn fence(info: &str, content: &str) -> String {
    let mut ticks = 3;
    let mut run = 0;
    for ch in content.chars() {
        if ch == '`' {
            run += 1;
            ticks = ticks.max(run + 1);
        } else {
            run = 0;
        }
    }
    let delimiter = "`".repeat(ticks);
    let mut block = format!("{delimiter}{info}\n{content}");
    if !content.ends_with('\n') {
        block.push('\n');
    }
    block.push_str(&delimiter);
    block
}

/// Mirrors `ui/src/format/body.ts` `bodyNotice`.
fn body_notice(body: &BodyCapture) -> Option<String> {
    if body.truncated {
        let kept = format_bytes(body.captured_bytes);
        let total = match body.size_bytes {
            Some(size) => format_bytes(size),
            None => kept.clone(),
        };
        return Some(format!("Body truncated at {kept} of {total}"));
    }
    if body.encoding != BodyEncoding::Omitted && body.omitted_reason.is_none() {
        return None;
    }
    let reason = body.omitted_reason.as_deref().unwrap_or("omitted");
    let size = format_bytes(body.size_bytes.unwrap_or(body.captured_bytes));
    if reason == "not_read_by_app" {
        return Some(format!("The app did not read this body ({size})"));
    }
    if reason == "binary" || reason.starts_with("binary:") {
        let kind = reason.strip_prefix("binary:").unwrap_or("binary");
        return Some(format!("Binary body omitted ({kind}, {size})"));
    }
    Some(format!("Body omitted ({reason}, {size})"))
}

/// Mirrors `ui/src/format/bytes.ts` `formatBytes` (base 1024, one decimal from KB).
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[derive(Default)]
struct Doc {
    lines: Vec<String>,
}

impl Doc {
    fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    fn blank(&mut self) {
        if self.lines.last().is_some_and(|line| !line.is_empty()) {
            self.lines.push(String::new());
        }
    }

    fn finish(self) -> String {
        let mut doc = self.lines.join("\n");
        doc.push('\n');
        doc
    }
}
