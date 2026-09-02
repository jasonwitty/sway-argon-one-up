//! Embed the git commit into the binary so a running daemon can be matched to
//! its source. Months of "which build is actually running?" triage (md5-summing
//! binaries, grepping them for magic strings, one wrong grep that falsely
//! reported a fix missing — see notes/trackpad-guard-engineering-log.md,
//! 2026-07-01) came from not having this. No dependencies: shells out to git,
//! and degrades to an empty value when built outside a checkout (vendored
//! tarball, no git on PATH).

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let hash = git(&["rev-parse", "--short=12", "HEAD"]);
    // Uncommitted changes get a "-dirty" suffix so an installed test build can
    // never masquerade as the commit it was based on.
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    let value = match hash {
        Some(h) if dirty => format!("{h}-dirty"),
        Some(h) => h,
        None => String::new(),
    };
    println!("cargo:rustc-env=TRACKPAD_GUARD_GIT_HASH={value}");

    // Rebuild when HEAD moves (or the index changes) so the embedded hash
    // cannot go stale across commits. A stale -dirty flag is still possible —
    // editing a tracked file does not retrigger build.rs by itself — but the
    // hash stays correct, and the install flow is a fresh `cargo build
    // --release` in practice.
    if let Some(gitdir) = git(&["rev-parse", "--git-dir"]) {
        println!("cargo:rerun-if-changed={gitdir}/HEAD");
        println!("cargo:rerun-if-changed={gitdir}/index");
    }
}
