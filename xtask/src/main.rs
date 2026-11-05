//! Workspace tasks for verification, the UI build, and screenshots.

#![forbid(unsafe_code)]

mod contrast;
mod hygiene;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Harness {
    milestone: String,
    validations: BTreeMap<String, Gate>,
    mandatory_tests: BTreeMap<String, MandatoryTest>,
    thresholds: Thresholds,
    dependencies: Dependencies,
}

#[derive(Debug, Deserialize)]
struct Gate {
    since: String,
}

#[derive(Debug, Deserialize)]
struct MandatoryTest {
    since: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct Thresholds {
    screenshot_threshold: f64,
    screenshot_max_diff_pixels: u32,
    bundle_js_css_gzip_max_bytes: u64,
    bundle_woff2_max_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct Dependencies {
    oxray: DepKind,
    demo: DepKind,
    xtask: DepKind,
    ui: DepKind,
    versions: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct DepKind {
    runtime: Vec<String>,
    #[serde(default)]
    dev: Vec<String>,
}

const PLAYWRIGHT_FILES: [&str; 3] = [
    "shell.spec.ts",
    "a11y.spec.ts",
    "mandatory/no-external-requests.spec.ts",
];

const VALIDATIONS: [&str; 19] = [
    "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13", "v14", "v15",
    "v16", "v17", "v18", "v19",
];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .context("usage: xtask <verify|ui-build|screenshots|seed|bench>")?;
    let root = workspace_root()?;
    match command.as_str() {
        "seed" => seed(&root, args),
        "bench" => {
            if args.next().is_some() {
                bail!("unexpected extra argument");
            }
            bench(&root)
        }
        "verify" => {
            if args.next().is_some() {
                bail!("unexpected extra argument");
            }
            verify(&root)
        }
        "ui-build" => {
            if args.next().is_some() {
                bail!("unexpected extra argument");
            }
            ensure_node()?;
            npm_ci(&root)?;
            vite_build(&root)
        }
        "screenshots" => {
            if args.next().is_some() {
                bail!("unexpected extra argument");
            }
            ensure_node()?;
            run_visual(&root, true)
        }
        other => bail!("unknown command {other}"),
    }
}

fn seed(root: &Path, mut args: impl Iterator<Item = String>) -> Result<()> {
    let flag = args.next().context("usage: xtask seed --url <url>")?;
    if flag != "--url" {
        bail!("usage: xtask seed --url <url>");
    }
    let url = args.next().context("usage: xtask seed --url <url>")?;
    if args.next().is_some() {
        bail!("unexpected extra argument");
    }
    cargo(root, &["run", "-p", "demo", "--", "seed", "--url", &url])
}

fn bench(root: &Path) -> Result<()> {
    let baseline = bench_case(root, "baseline", false)?;
    let disabled = bench_case(root, "disabled", true)?;
    let dev = bench_case(root, "dev", false)?;
    writeln_stdout(&format!("baseline_ns {baseline}"))?;
    writeln_stdout(&format!("disabled_ns {disabled}"))?;
    writeln_stdout(&format!("dev_ns {dev}"))?;
    writeln_stdout(&format!(
        "M-001 disabled_overhead_ns {}",
        disabled.saturating_sub(baseline)
    ))?;
    writeln_stdout(&format!(
        "M-002 dev_overhead_ns {}",
        dev.saturating_sub(baseline)
    ))?;
    Ok(())
}

fn bench_case(root: &Path, case: &str, disabled: bool) -> Result<u128> {
    let mut command = Command::new("cargo");
    command
        .args([
            "bench",
            "--profile",
            "bench-dev",
            "-p",
            "oxray",
            "--bench",
            "overhead",
        ])
        .current_dir(root)
        .env("OXRAY_BENCH", case);
    if disabled {
        command.env("OXRAY", "off");
    } else {
        command.env_remove("OXRAY");
    }
    let result = command.output().context("spawn cargo bench")?;
    let stdout = String::from_utf8(result.stdout).context("bench output")?;
    if !result.status.success() {
        bail!(
            "bench {case} exited {}\n{}\n{stdout}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
    }
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("median_ns "))
        .context("bench did not print median_ns")?
        .trim()
        .parse()
        .context("median_ns")
}

fn workspace_root() -> Result<PathBuf> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()?)
}

fn verify(root: &Path) -> Result<()> {
    let harness = load_harness(root)?;
    lock_thresholds(&harness)?;
    let current = rank(&harness.milestone)?;
    ensure_node()?;
    npm_ci(root)?;
    for name in VALIDATIONS {
        let gate = harness
            .validations
            .get(name)
            .with_context(|| format!("harness.toml is missing validation {name}"))?;
        if rank(&gate.since)? <= current {
            writeln_stdout(&format!("== {name}"))?;
            run_validation(root, &harness, name)?;
        } else {
            writeln_stdout(&format!("== {name} skipped (since {})", gate.since))?;
        }
    }
    assert_active_tests(root, &harness)?;
    writeln_stdout("verify passed")?;
    Ok(())
}

fn run_validation(root: &Path, harness: &Harness, name: &str) -> Result<()> {
    match name {
        "v1" => format_all(root),
        "v2" => cargo(
            root,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "v3" => lint_ts(root),
        "v4" => test_rust(root),
        "v6" => vitest(root),
        "v7" => {
            install_browsers(root)?;
            playwright_functional(root)
        }
        "v8" | "v11" => playwright_report_has_required(root),
        "v19" => playwright_report_has_files(root, &["dom-lint.spec.ts"]),
        "v9" => visual(root),
        "v10" => contrast::check(root),
        "v12" => check_bundle(root, &harness.thresholds),
        "v13" => fresh_ui(root),
        "v14" => docs(root),
        "v15" => dependencies(root),
        "v16" => hygiene::check(root),
        "v5" => release_guard(root),
        "v18" => redaction_e2e(root),
        "v17" => bail!("{name} is active but has no runner"),
        other => bail!("unknown validation {other}"),
    }
}

fn format_all(root: &Path) -> Result<()> {
    cargo(root, &["fmt", "--all", "--", "--check"])?;
    command(
        root,
        "npx",
        &["biome", "format", "."],
        Some(&root.join("ui")),
    )
}

fn lint_ts(root: &Path) -> Result<()> {
    let ui = root.join("ui");
    command(root, "npx", &["biome", "lint", "."], Some(&ui))?;
    command(root, "npx", &["tsc", "--noEmit"], Some(&ui))
}

fn test_rust(root: &Path) -> Result<()> {
    cargo(root, &["test", "--workspace", "--all-features"])?;
    cargo(root, &["test", "-p", "oxray", "--no-default-features"])?;
    let listed = output(
        root,
        "cargo",
        &["test", "-p", "oxray", "--all-features", "--", "--list"],
    )?;
    let cases = listed
        .lines()
        .filter(|line| line.contains(": test"))
        .count();
    if cases == 0 {
        bail!("oxray listed zero tests");
    }
    Ok(())
}

fn release_guard(root: &Path) -> Result<()> {
    cargo(
        root,
        &[
            "test",
            "-p",
            "oxray",
            "--release",
            "--test",
            "release_guard",
        ],
    )?;
    let listed = output(
        root,
        "cargo",
        &[
            "test",
            "-p",
            "oxray",
            "--release",
            "--test",
            "release_guard",
            "--",
            "--list",
        ],
    )?;
    let cases = listed
        .lines()
        .filter(|line| line.contains(": test"))
        .count();
    if cases == 0 {
        bail!("release_guard listed zero tests");
    }
    Ok(())
}

fn redaction_e2e(root: &Path) -> Result<()> {
    let path = root.join("crates/oxray/tests/mandatory/redaction_e2e.rs");
    let text = fs::read_to_string(&path).with_context(|| format!("missing {}", path.display()))?;
    if !text.contains("#[test]") && !text.contains("#[tokio::test]") {
        bail!("redaction_e2e has no tests");
    }
    cargo(root, &["test", "-p", "oxray", "--test", "redaction_e2e"])?;
    cargo(
        root,
        &["test", "-p", "demo", "corpus_route_redacts_every_surface"],
    )?;
    let harness = load_harness(root)?;
    if rank(&harness.milestone)? >= rank("M3b")? {
        playwright_report_has_files(root, &["e2e/flows.spec.ts"])?;
    }
    Ok(())
}

fn vitest(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("target"))?;
    command(
        root,
        "npx",
        &[
            "vitest",
            "run",
            "--reporter=json",
            "--outputFile",
            "../target/vitest.json",
        ],
        Some(&root.join("ui")),
    )?;
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("target/vitest.json"))?)?;
    let total = report
        .get("numTotalTests")
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    if total == 0 {
        bail!("vitest ran zero tests");
    }
    Ok(())
}

fn fresh_ui(root: &Path) -> Result<()> {
    vite_build(root)?;
    let status = output(
        root,
        "git",
        &["status", "--porcelain", "--", "crates/oxray/ui-dist"],
    )?;
    if !status.trim().is_empty() {
        bail!("ui-dist does not match the source build:\n{status}");
    }
    Ok(())
}

fn visual(root: &Path) -> Result<()> {
    run_visual(root, false)?;
    let screenshots = count_pngs(root)?;
    if screenshots < 6 {
        bail!("expected screenshots for both themes and three widths, found {screenshots}");
    }
    Ok(())
}

fn docs(root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .current_dir(root)
        .env("RUSTDOCFLAGS", "-D warnings")
        .args(["doc", "-p", "oxray", "--no-deps", "--all-features"])
        .status()
        .context("cargo doc")?;
    if !status.success() {
        bail!("cargo doc exited {status}");
    }
    Ok(())
}

fn dependencies(root: &Path) -> Result<()> {
    ensure_cargo_deny(root)?;
    cargo(root, &["deny", "check"])?;
    check_dependency_lists(root)
}

fn ensure_cargo_deny(root: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .args(["deny", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if matches!(status, Ok(status) if status.success()) {
        return Ok(());
    }
    command(root, "cargo", &["install", "cargo-deny", "--locked"], None)
}

fn lock_thresholds(harness: &Harness) -> Result<()> {
    let threshold = (harness.thresholds.screenshot_threshold - 0.1).abs() < 1.0e-9;
    if !threshold || harness.thresholds.screenshot_max_diff_pixels != 100 {
        bail!("screenshot threshold must stay 0.1 and maxDiffPixels 100");
    }
    if harness.thresholds.bundle_js_css_gzip_max_bytes != 153_600
        || harness.thresholds.bundle_woff2_max_bytes != 153_600
    {
        bail!("bundle ceilings must stay 150 KiB");
    }
    Ok(())
}

fn load_harness(root: &Path) -> Result<Harness> {
    let path = root.join("harness.toml");
    let text = fs::read_to_string(&path)?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn assert_active_tests(root: &Path, harness: &Harness) -> Result<()> {
    let current = rank(&harness.milestone)?;
    let mut active = 0_u32;
    for (name, test) in &harness.mandatory_tests {
        if rank(&test.since)? > current {
            continue;
        }
        active += 1;
        let path = root.join(&test.path);
        let text =
            fs::read_to_string(&path).with_context(|| format!("missing mandatory test {name}"))?;
        let has_case = text.contains("#[test]")
            || text.contains("#[tokio::test]")
            || text.contains("test(")
            || text.contains("it(");
        if !has_case {
            bail!("mandatory test {name} has no cases");
        }
    }
    if active == 0 {
        bail!(
            "milestone {} has no active mandatory tests",
            harness.milestone
        );
    }
    Ok(())
}

fn rank(milestone: &str) -> Result<u8> {
    Ok(match milestone {
        "M0" => 0,
        "M1" => 1,
        "M2" => 2,
        "M3a" => 3,
        "M3b" => 4,
        "M4" => 5,
        "M5" => 6,
        "M6" => 7,
        "M7" => 8,
        "M8" => 9,
        "M9" => 10,
        other => bail!("unknown milestone {other}"),
    })
}

fn ensure_node() -> Result<()> {
    let version = output_at(Path::new("."), "node", &["-v"])?;
    let version = version.trim();
    if !version.starts_with("v24.") {
        bail!("Node 24 is required, found {version}");
    }
    writeln_stdout(&format!("node {version}"))?;
    Ok(())
}

fn npm_ci(root: &Path) -> Result<()> {
    command(root, "npm", &["ci"], Some(&root.join("ui")))
}

fn vite_build(root: &Path) -> Result<()> {
    command(root, "npx", &["vite", "build"], Some(&root.join("ui")))
}

fn install_browsers(root: &Path) -> Result<()> {
    let mut args = vec!["playwright", "install"];
    if cfg!(target_os = "linux") {
        args.push("--with-deps");
    }
    args.push("chromium");
    command(root, "npx", &args, Some(&root.join("ui")))
}

fn playwright_functional(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("target"))?;
    let report_path = root.join("target/playwright.json");
    let status = Command::new("npx")
        .current_dir(root.join("ui"))
        .env(
            "OXRAY_PLAYWRIGHT_JSON",
            report_path.to_str().context("playwright report path")?,
        )
        .env("OXRAY_E2E", "1")
        .args(["playwright", "test", "--grep-invert", "@visual"])
        .status()
        .context("playwright")?;
    if !status.success() {
        bail!("playwright exited {status}");
    }
    let report: serde_json::Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
    let mut passed: BTreeMap<String, u64> = BTreeMap::new();
    walk_suites(&report, &mut passed);
    require_named(&passed, &PLAYWRIGHT_FILES)
}

fn playwright_report_has_required(root: &Path) -> Result<()> {
    playwright_report_has_files(root, &PLAYWRIGHT_FILES)
}

fn playwright_report_has_files(root: &Path, required: &[&str]) -> Result<()> {
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("target/playwright.json"))?)
            .context("playwright report from v7")?;
    let mut passed = BTreeMap::new();
    walk_suites(&report, &mut passed);
    require_named(&passed, required)
}

fn require_named(passed: &BTreeMap<String, u64>, required: &[&str]) -> Result<()> {
    for name in required {
        let cases = passed
            .iter()
            .find(|(file, _)| file.replace('\\', "/").ends_with(name))
            .map(|(_, count)| *count)
            .unwrap_or(0);
        if cases == 0 {
            bail!("playwright ran zero tests for {name}");
        }
    }
    Ok(())
}

fn walk_suites(node: &serde_json::Value, passed: &mut BTreeMap<String, u64>) {
    if let Some(file) = node.get("file").and_then(|value| value.as_str())
        && let Some(specs) = node.get("specs").and_then(|value| value.as_array())
    {
        for spec in specs {
            if spec.get("ok").and_then(|value| value.as_bool()) != Some(true) {
                continue;
            }
            let count = spec
                .get("tests")
                .and_then(|value| value.as_array())
                .map(|tests| tests.len())
                .unwrap_or(0);
            *passed.entry(file.to_owned()).or_default() += u64::try_from(count).unwrap_or(0);
        }
    }
    if let Some(suites) = node.get("suites").and_then(|value| value.as_array()) {
        for suite in suites {
            walk_suites(suite, passed);
        }
    }
}

fn run_visual(root: &Path, update: bool) -> Result<()> {
    let docker = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .context("docker")?;
    if !docker.status.success() {
        bail!("Docker is required for visual checks");
    }
    let image = playwright_image(root)?;
    let uid = output_at(Path::new("."), "id", &["-u"])?;
    let gid = output_at(Path::new("."), "id", &["-g"])?;
    let script = "set -eu
npm ci --engine-strict=false
status=0
if [ \"$OXRAY_UPDATE_SNAPSHOTS\" = \"1\" ]; then
  npx playwright test --grep @visual --update-snapshots || status=$?
else
  npx playwright test --grep @visual || status=$?
fi
if [ -d tests/screenshots ]; then
  chown -R \"$HOST_UID:$HOST_GID\" tests/screenshots
fi
if [ -d test-results ]; then
  chown -R \"$HOST_UID:$HOST_GID\" test-results
fi
exit \"$status\"";
    let mut command = Command::new("docker");
    command
        .arg("run")
        .arg("--rm")
        .arg("-e")
        .arg("HOME=/tmp")
        .arg("-e")
        .arg("CI=1")
        .arg("-e")
        .arg("PLAYWRIGHT_BROWSERS_PATH=/ms-playwright")
        .arg("-e")
        .arg(format!("HOST_UID={}", uid.trim()))
        .arg("-e")
        .arg(format!("HOST_GID={}", gid.trim()))
        .arg("-e")
        .arg(format!(
            "OXRAY_UPDATE_SNAPSHOTS={}",
            if update { "1" } else { "0" }
        ))
        .arg("-v")
        .arg(format!("{}:/work", root.display()))
        .arg("-v")
        .arg("oxray-ui-node-modules:/work/ui/node_modules")
        .arg("-w")
        .arg("/work/ui")
        .arg(&image)
        .arg("bash")
        .arg("-lc")
        .arg(script);
    let status = command.status().context("docker run")?;
    if !status.success() {
        bail!("playwright visual checks exited {status}");
    }
    Ok(())
}

fn playwright_image(root: &Path) -> Result<String> {
    let lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("ui/package-lock.json"))?)?;
    let version = lock
        .get("packages")
        .and_then(|packages| packages.get("node_modules/@playwright/test"))
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .context("@playwright/test version in the lockfile")?;
    Ok(format!("mcr.microsoft.com/playwright:v{version}-noble"))
}

fn count_pngs(root: &Path) -> Result<usize> {
    let dir = root.join("ui/tests/screenshots");
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0_usize;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        for nested in fs::read_dir(entry.path())? {
            let nested = nested?;
            if nested.path().extension().is_some_and(|ext| ext == "png") {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn check_bundle(root: &Path, thresholds: &Thresholds) -> Result<()> {
    let dist = root.join("crates/oxray/ui-dist");
    let html = fs::read_to_string(dist.join("index.html")).context("built index.html")?;
    let attributes = regex::Regex::new(r#"(?:src|href)="([^"]+)""#)?;
    let mut scripts = Vec::new();
    for found in attributes.captures_iter(&html) {
        let reference = found.get(1).context("asset reference")?.as_str();
        if reference.ends_with(".js") || reference.ends_with(".css") {
            scripts.push(dist.join(reference.trim_start_matches("./")));
        }
    }
    if scripts.is_empty() {
        bail!("index.html references no JS or CSS");
    }
    let mut gzip_total = 0_u64;
    let mut woff_total = 0_u64;
    let urls = regex::Regex::new(r#"url\((?:["']?)([^"')]+)(?:["']?)\)"#)?;
    for asset in &scripts {
        let bytes = fs::read(asset).with_context(|| format!("read {}", asset.display()))?;
        gzip_total += gzip_len(&bytes)?;
        if asset.extension().is_some_and(|ext| ext == "css") {
            let css = String::from_utf8(bytes)?;
            let base = asset.parent().context("css directory")?;
            for found in urls.captures_iter(&css) {
                let reference = found.get(1).context("font url")?.as_str();
                if !reference.contains(".woff2") {
                    continue;
                }
                let font = base.join(reference.trim_start_matches("./"));
                let size = fs::metadata(&font)
                    .with_context(|| format!("font {}", font.display()))?
                    .len();
                woff_total += size;
            }
        }
    }
    if gzip_total > thresholds.bundle_js_css_gzip_max_bytes {
        bail!("JS and CSS gzip is {gzip_total} bytes");
    }
    if woff_total > thresholds.bundle_woff2_max_bytes {
        bail!("first-paint woff2 is {woff_total} bytes");
    }
    Ok(())
}

fn gzip_len(bytes: &[u8]) -> Result<u64> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes)?;
    let compressed = encoder.finish()?;
    u64::try_from(compressed.len()).context("gzip length")
}

fn check_dependency_lists(root: &Path) -> Result<()> {
    let harness = load_harness(root)?;
    let workspace: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    if workspace
        .get("workspace")
        .and_then(|item| item.get("dependencies"))
        .is_some()
    {
        bail!("declare dependencies on each crate, not in [workspace.dependencies]");
    }
    let members = workspace
        .get("workspace")
        .and_then(|item| item.get("members"))
        .and_then(|item| item.as_array())
        .context("workspace members")?;
    for member in members {
        let name = member.as_str().context("workspace member")?;
        let (runtime, dev) = allow_lists(&harness.dependencies, name)?;
        check_manifest(
            &root.join(name).join("Cargo.toml"),
            runtime,
            dev,
            &harness.dependencies.versions,
        )?;
    }
    check_package_json(root, &harness.dependencies)
}

fn allow_lists<'a>(deps: &'a Dependencies, member: &str) -> Result<(&'a [String], &'a [String])> {
    let kind = match member {
        "crates/oxray" => &deps.oxray,
        "examples/demo" => &deps.demo,
        "xtask" => &deps.xtask,
        other => bail!("no dependency allow list for {other}"),
    };
    Ok((&kind.runtime, &kind.dev))
}

fn check_manifest(
    path: &Path,
    runtime: &[String],
    dev: &[String],
    versions: &BTreeMap<String, String>,
) -> Result<()> {
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(path)?)?;
    check_dep_table(path, manifest.get("dependencies"), runtime, versions)?;
    check_dep_table(path, manifest.get("dev-dependencies"), dev, versions)?;
    check_dep_table(path, manifest.get("build-dependencies"), runtime, versions)?;
    Ok(())
}

fn check_dep_table(
    path: &Path,
    table: Option<&toml::Value>,
    allow: &[String],
    versions: &BTreeMap<String, String>,
) -> Result<()> {
    let Some(table) = table.and_then(|value| value.as_table()) else {
        return Ok(());
    };
    for (name, value) in table {
        if !allow.iter().any(|allowed| allowed == name) {
            bail!(
                "{} depends on {name}, which is outside the allow list",
                path.display()
            );
        }
        if let Some(requirement) = requirement(value)
            && !version_allowed(name, requirement, versions)
        {
            bail!(
                "{} dependency {name} {requirement} is outside the allowed version",
                path.display()
            );
        }
    }
    Ok(())
}

fn requirement(value: &toml::Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("version").and_then(|item| item.as_str()))
}

fn version_allowed(name: &str, requirement: &str, versions: &BTreeMap<String, String>) -> bool {
    let Some(prefix) = versions.get(name) else {
        return true;
    };
    let trimmed = requirement.trim().trim_start_matches(['^', '~', '=']);
    trimmed == prefix.as_str() || trimmed.starts_with(&format!("{prefix}."))
}

fn check_package_json(root: &Path, deps: &Dependencies) -> Result<()> {
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("ui/package.json"))?)?;
    check_npm_section(
        package.get("dependencies"),
        &deps.ui.runtime,
        &deps.versions,
        "ui runtime",
    )?;
    check_npm_section(
        package.get("devDependencies"),
        &deps.ui.dev,
        &deps.versions,
        "ui dev",
    )?;
    Ok(())
}

fn check_npm_section(
    section: Option<&serde_json::Value>,
    allow: &[String],
    versions: &BTreeMap<String, String>,
    label: &str,
) -> Result<()> {
    let Some(section) = section.and_then(|value| value.as_object()) else {
        return Ok(());
    };
    for (name, requirement) in section {
        if !allow.iter().any(|allowed| allowed == name) {
            bail!("{label} depends on {name}, which is outside the allow list");
        }
        let Some(requirement) = requirement.as_str() else {
            bail!("{label} dependency {name} has no version");
        };
        if !version_allowed(name, requirement, versions) {
            bail!("{label} dependency {name} {requirement} is outside the allowed version");
        }
    }
    Ok(())
}

fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    command(root, "cargo", args, None)
}

fn command(root: &Path, program: &str, args: &[&str], dir: Option<&Path>) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir.unwrap_or(root))
        .status()
        .with_context(|| format!("spawn {program}"))?;
    if !status.success() {
        bail!("{program} {} exited {status}", args.join(" "));
    }
    Ok(())
}

fn output(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    output_at(root, program, args)
}

fn output_at(dir: &Path, program: &str, args: &[&str]) -> Result<String> {
    let result = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("spawn {program}"))?;
    if !result.status.success() {
        bail!("{program} {} exited {}", args.join(" "), result.status);
    }
    Ok(String::from_utf8(result.stdout)?)
}

fn writeln_stdout(message: &str) -> Result<()> {
    use std::io::Write;
    writeln!(std::io::stdout(), "{message}")?;
    Ok(())
}
