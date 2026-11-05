//! Measures the contrast pairs declared in `harness.toml` against `tokens.css`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ContrastFile {
    contrast: Contrast,
}

#[derive(Debug, Deserialize)]
struct Contrast {
    pairs: Vec<Pair>,
}

#[derive(Debug, Deserialize)]
struct Pair {
    name: String,
    kind: String,
    fg: Option<String>,
    fg_light: Option<String>,
    fg_dark: Option<String>,
    bg: String,
    tint: Option<String>,
    min: f64,
}

#[derive(Clone, Copy)]
struct Rgb {
    r: f64,
    g: f64,
    b: f64,
}

pub fn check(root: &Path) -> Result<()> {
    let harness_path = root.join("harness.toml");
    let css_path = root.join("ui/src/styles/tokens.css");
    let harness: ContrastFile = toml::from_str(&fs::read_to_string(&harness_path)?)
        .with_context(|| format!("parse {}", harness_path.display()))?;
    let css = fs::read_to_string(&css_path)?;
    let light = colors_in(&css, ":root")?;
    let dark = colors_in(&css, "[data-theme=\"dark\"]")?;
    if harness.contrast.pairs.is_empty() {
        bail!("contrast matrix has no pairs");
    }
    let mut failures = Vec::new();
    for pair in &harness.contrast.pairs {
        let floor = match pair.kind.as_str() {
            "text" => 4.5,
            "control" => 3.0,
            other => bail!("contrast pair {} has unknown kind {other}", pair.name),
        };
        let required = pair.min.max(floor);
        for (theme, colors) in [("light", &light), ("dark", &dark)] {
            let fg_name = foreground(pair, theme)?;
            let fg = color(colors, fg_name, theme, &pair.name)?;
            let bg = color(colors, &pair.bg, theme, &pair.name)?;
            let behind = match &pair.tint {
                Some(tint) => {
                    let tint_color = color(colors, tint, theme, &pair.name)?;
                    mix(tint_color, bg, 0.1)
                }
                None => bg,
            };
            let ratio = contrast(fg, behind);
            if ratio < required {
                failures.push(format!(
                    "{} {theme} {fg_name} on {} = {ratio:.2} (need {required:.2})",
                    pair.name,
                    pair.tint.as_deref().unwrap_or(pair.bg.as_str()),
                ));
            }
        }
    }
    if !failures.is_empty() {
        bail!("contrast failed:\n{}", failures.join("\n"));
    }
    Ok(())
}

fn foreground<'a>(pair: &'a Pair, theme: &str) -> Result<&'a str> {
    if let Some(fg) = &pair.fg {
        return Ok(fg);
    }
    let specific = if theme == "light" {
        pair.fg_light.as_deref()
    } else {
        pair.fg_dark.as_deref()
    };
    specific.with_context(|| format!("contrast pair {} is missing a foreground", pair.name))
}

fn color(colors: &BTreeMap<String, Rgb>, name: &str, theme: &str, pair: &str) -> Result<Rgb> {
    colors
        .get(name)
        .copied()
        .with_context(|| format!("contrast pair {pair} is missing {theme} token {name}"))
}

fn colors_in(css: &str, selector: &str) -> Result<BTreeMap<String, Rgb>> {
    let marker = format!("{selector} {{");
    let mut colors = BTreeMap::new();
    let mut search_from = 0;
    while let Some(relative) = css[search_from..].find(&marker) {
        let body_start = search_from + relative + marker.len();
        let body = block_body(&css[body_start..], selector)?;
        collect_colors(body, &mut colors);
        search_from = body_start + body.len();
    }
    if colors.is_empty() {
        bail!("no colors found for {selector}");
    }
    Ok(colors)
}

fn collect_colors(body: &str, colors: &mut BTreeMap<String, Rgb>) {
    for line in body.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().trim_start_matches("--color-");
        let value = value.trim().trim_end_matches(';').trim();
        if name.is_empty() || !value.starts_with('#') {
            continue;
        }
        if let Some(rgb) = parse_hex(value) {
            colors.insert(name.to_owned(), rgb);
        }
    }
}

fn block_body<'a>(css: &'a str, selector: &str) -> Result<&'a str> {
    let mut depth = 1_i32;
    for (index, character) in css.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(&css[..index]);
                }
            }
            _ => {}
        }
    }
    bail!("unclosed {selector} in tokens.css")
}

fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let channel = |offset: usize| u8::from_str_radix(&hex[offset..offset + 2], 16).ok();
    Some(Rgb {
        r: f64::from(channel(0)?),
        g: f64::from(channel(2)?),
        b: f64::from(channel(4)?),
    })
}

fn mix(foreground: Rgb, background: Rgb, amount: f64) -> Rgb {
    let blend = |front: f64, back: f64| front * amount + back * (1.0 - amount);
    Rgb {
        r: blend(foreground.r, background.r),
        g: blend(foreground.g, background.g),
        b: blend(foreground.b, background.b),
    }
}

fn contrast(first: Rgb, second: Rgb) -> f64 {
    let first = luminance(first);
    let second = luminance(second);
    let (high, low) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    (high + 0.05) / (low + 0.05)
}

fn luminance(color: Rgb) -> f64 {
    let channel = |value: f64| {
        let scaled = value / 255.0;
        if scaled <= 0.04045 {
            scaled / 12.92
        } else {
            ((scaled + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_passes_for_both_themes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        check(&root).unwrap();
    }
}
