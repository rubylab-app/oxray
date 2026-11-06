//! Checks the README install blocks and the packaged crate.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use regex::Regex;

pub fn package(root: &Path) -> Result<()> {
    let version = package_version(&root.join("crates/oxray/Cargo.toml"))?;
    let result = Command::new("cargo")
        .args(["package", "-p", "oxray", "--locked", "--offline"])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .context("cargo package")?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    if !result.status.success() {
        bail!("cargo package exited {}\n{stdout}\n{stderr}", result.status);
    }
    if stdout.contains("warning:") || stderr.contains("warning:") {
        bail!("cargo package was not clean\n{stdout}\n{stderr}");
    }
    let packed = root.join(format!("target/package/oxray-{version}.crate"));
    let bytes = fs::metadata(&packed)
        .with_context(|| format!("missing {}", packed.display()))?
        .len();
    if bytes >= 2_000_000 {
        bail!("oxray-{version}.crate is {bytes} bytes");
    }
    Ok(())
}

/// Builds a fresh `cargo new` project from the README blocks and checks that
/// a request shows up in the capture API.
pub fn install(root: &Path) -> Result<()> {
    let readme = fs::read_to_string(root.join("README.md")).context("README.md")?;
    let blocks = install_blocks(&readme)?;
    let source = example_source(&blocks)?;
    let oxray = root.join("crates/oxray").canonicalize()?;
    let dependencies = dependency_block(&readme, &oxray)?;
    let name = format!("oxray-readme-{}", std::process::id());
    let project = std::env::temp_dir().join(&name);
    let _cleanup = RemoveDir(project.clone());
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    let status = Command::new("cargo")
        .args([
            "new",
            "--vcs",
            "none",
            project.to_str().context("temp path")?,
        ])
        .status()
        .context("cargo new")?;
    if !status.success() {
        bail!("cargo new exited {status}");
    }
    fs::write(project.join("src/main.rs"), &source)?;
    let manifest = project.join("Cargo.toml");
    let generated = fs::read_to_string(&manifest)?;
    let package = generated
        .split("[dependencies]")
        .next()
        .context("generated manifest")?;
    fs::write(&manifest, format!("{package}{dependencies}\n"))?;
    // Same versions as the workspace, so the shared target dir reuses most
    // artifacts instead of compiling a second graph.
    fs::copy(root.join("Cargo.lock"), project.join("Cargo.lock"))?;
    run_example(root, &manifest, &name)
}

fn package_version(manifest: &Path) -> Result<String> {
    let text = fs::read_to_string(manifest)?;
    text.lines()
        .find_map(|line| {
            let rest = line.strip_prefix("version = \"")?;
            Some(rest.trim_end_matches('"').to_owned())
        })
        .context("oxray version")
}

fn install_blocks(readme: &str) -> Result<Vec<String>> {
    let pattern = Regex::new(r"(?ms)^```rust oxray-install\n(.*?)\n```$")?;
    let blocks: Vec<String> = pattern
        .captures_iter(readme)
        .filter_map(|caps| caps.get(1).map(|item| item.as_str().to_owned()))
        .filter(|block| !block.trim().is_empty())
        .collect();
    if blocks.is_empty() {
        bail!("README has no rust oxray-install blocks");
    }
    Ok(blocks)
}

fn example_source(blocks: &[String]) -> Result<String> {
    let mut mains = blocks.iter().filter(|block| block.contains("fn main"));
    let Some(main) = mains.next() else {
        bail!("README install blocks have no fn main");
    };
    if mains.next().is_some() {
        bail!("README has more than one install block with fn main");
    }
    let lines: Vec<&str> = main.lines().map(str::trim).collect();
    for block in blocks.iter().filter(|block| !block.contains("fn main")) {
        for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if !lines.contains(&line) {
                bail!("install line is missing from the example: {line}");
            }
        }
    }
    Ok(main.clone())
}

/// The README `toml oxray-install` block, with `oxray` pointed at this checkout.
fn dependency_block(readme: &str, oxray: &Path) -> Result<String> {
    let path = oxray.to_str().context("oxray path")?;
    if path.contains('"') {
        bail!("oxray path contains a quote");
    }
    let pattern = Regex::new(r"(?ms)^```toml oxray-install\n(.*?)\n```$")?;
    let mut found = pattern.captures_iter(readme);
    let block = found
        .next()
        .and_then(|caps| caps.get(1))
        .context("README has no toml oxray-install block")?
        .as_str();
    if found.next().is_some() {
        bail!("README has more than one toml oxray-install block");
    }
    if !block.trim_start().starts_with("[dependencies]") {
        bail!("the toml oxray-install block must start with [dependencies]");
    }
    let mut replaced = false;
    let lines: Vec<String> = block
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("oxray =") {
                replaced = true;
                format!("oxray = {{ path = \"{path}\" }}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !replaced {
        bail!("the toml oxray-install block has no oxray dependency");
    }
    Ok(lines.join("\n"))
}

fn run_example(root: &Path, manifest: &Path, name: &str) -> Result<()> {
    let target = root.join("target");
    // Run the built binary so `Child::kill` stops it. Ubuntu 24.04 reads
    // `kill -KILL -<pgid>` with a leading 1 as `-1`, which kills every
    // process of the user, including the CI runner.
    let status = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(manifest)
        .current_dir(root)
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .context("cargo build")?;
    if !status.success() {
        bail!("building the README example exited {status}");
    }
    let exe = target
        .join("debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let mut child = Command::new(&exe)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {}", exe.display()))?;
    let stdout = child.stdout.take().context("example stdout")?;
    let _running = Running(child);
    let (lines, heard) = mpsc::channel();
    // Keep draining stdout after the address arrives so the example never
    // blocks on a full pipe.
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else {
                break;
            };
            let _ = lines.send(line);
        }
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut log = String::new();
    let addr = loop {
        match heard.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                if let Some(addr) = line.strip_prefix("listening on http://") {
                    break addr.to_owned();
                }
                log.push_str(&line);
                log.push('\n');
            }
            Err(RecvTimeoutError::Timeout) => {
                bail!("example did not listen within 60 s\n{log}");
            }
            Err(RecvTimeoutError::Disconnected) => {
                bail!("example exited before listening\n{log}");
            }
        }
    };
    let home = http_get(&addr, "/")?;
    if !home.starts_with("HTTP/1.1 200") {
        bail!("example request failed\n{home}");
    }
    let captures = http_get(&addr, "/_oxray/api/captures")?;
    if !captures.starts_with("HTTP/1.1 200") || !captures.contains("\"path\":\"/\"") {
        bail!("capture list did not include the request\n{captures}");
    }
    Ok(())
}

fn http_get(addr: &str, path: &str) -> Result<String> {
    let mut stream = TcpStream::connect(addr).with_context(|| format!("connect {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )?;
    let mut body = String::new();
    stream.read_to_string(&mut body)?;
    Ok(body)
}

/// Stops the example when the check ends, whichever way it ends.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct RemoveDir(PathBuf);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{dependency_block, example_source, install_blocks};

    #[test]
    fn readme_install_blocks_agree() {
        let readme = include_str!("../../README.md");
        let blocks = install_blocks(readme).expect("blocks");
        assert!(example_source(&blocks).expect("source").contains("fn main"));
        let deps = dependency_block(readme, Path::new("/tmp/oxray")).expect("deps");
        assert!(deps.contains("oxray = { path = \"/tmp/oxray\" }"));
    }
}
