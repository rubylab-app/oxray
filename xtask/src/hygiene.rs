//! Scans tracked files for patterns the harness rejects.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HygieneFile {
    hygiene: Hygiene,
}

#[derive(Debug, Deserialize)]
struct Hygiene {
    exclude: Vec<String>,
}

struct Rule {
    pattern: Regex,
    ui_only: bool,
}

pub fn check(root: &Path) -> Result<()> {
    let harness: HygieneFile = toml::from_str(&fs::read_to_string(root.join("harness.toml"))?)?;
    let tracked = tracked_files(root)?;
    let markdown = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "*.md"])
        .output()
        .context("git ls-files")?;
    if !markdown.status.success() {
        bail!("git ls-files failed");
    }
    let markdown = String::from_utf8(markdown.stdout)?;
    let markdown_files: Vec<_> = markdown.lines().filter(|line| !line.is_empty()).collect();
    if markdown_files != ["README.md"] {
        bail!("tracked markdown must be only README.md, found {markdown_files:?}");
    }
    let rules = rules()?;
    let mut hits = Vec::new();
    for path in &tracked {
        if excluded(path, &harness.hygiene.exclude) || is_binary(root, path)? {
            continue;
        }
        let text = fs::read_to_string(root.join(path))?;
        for rule in &rules {
            if rule.ui_only && !path.starts_with("ui/") {
                continue;
            }
            if rule.pattern.is_match(&text) {
                hits.push(format!("{path} matches {}", rule.pattern.as_str()));
            }
        }
    }
    if !hits.is_empty() {
        bail!("hygiene failed:\n{}", hits.join("\n"));
    }
    Ok(())
}

fn rules() -> Result<Vec<Rule>> {
    let compiled = [
        (r"\b(TODO|FIXME|XXX|HACK)\b", false),
        (r"#\[ignore", false),
        (r"cfg_attr\([^)]*ignore", false),
        (r"#!?\[allow\(", false),
        (
            r"\b(test|it|describe)(\.describe)?\.(skip|only|fixme)\(",
            true,
        ),
        (r"passWithNoTests", false),
        (r"retries:\s*[1-9]", false),
        (r"--ignore-rust-version", false),
        (r"set_var\(", false),
    ];
    compiled
        .into_iter()
        .map(|(pattern, ui_only)| {
            Ok(Rule {
                pattern: Regex::new(pattern)?,
                ui_only,
            })
        })
        .collect()
}

fn tracked_files(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .output()
        .context("git ls-files")?;
    if !output.status.success() {
        bail!("git ls-files failed");
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect())
}

fn excluded(path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| matches_glob(path, pattern))
}

fn matches_glob(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    let path_parts: Vec<&str> = path.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    if path_parts.len() != pattern_parts.len() {
        return false;
    }
    path_parts
        .iter()
        .zip(pattern_parts)
        .all(|(path_part, pattern_part)| wildcard(path_part, pattern_part))
}

fn wildcard(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return text == pattern;
    }
    let (prefix, suffix) = pattern.split_once('*').unwrap_or((pattern, ""));
    text.starts_with(prefix) && text.ends_with(suffix) && text.len() >= prefix.len() + suffix.len()
}

fn is_binary(root: &Path, path: &str) -> Result<bool> {
    let bytes = fs::read(root.join(path))?;
    Ok(bytes.contains(&0))
}
