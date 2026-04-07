fn main() {
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let is_nightly = std::env::var("AXE_NIGHTLY").is_ok();

    // Try `git describe --tags` first: yields "v0.1.0" (on tag) or "v0.1.0-5-gabc123" (after tag).
    let git_describe = std::process::Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short=6", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let version = if is_nightly {
        // Nightly: take base tag version, append -nightly-<hash>.
        let base = base_version_from_describe(&git_describe, &pkg_version);
        format!("{base}-nightly-{hash}")
    } else if let Some(ref desc) = git_describe {
        if desc.starts_with('v') && !desc.contains('-') {
            // Exactly on a tag: "v0.1.0"
            desc.clone()
        } else if desc.starts_with('v') && desc.contains('-') {
            // After a tag: "v0.1.0-5-gabc123" → "v0.1.0-dev.5-<hash>"
            let parts: Vec<&str> = desc.rsplitn(3, '-').collect();
            if parts.len() == 3 {
                let tag = parts[2];
                let commits = parts[1];
                format!("{tag}-dev.{commits}-{hash}")
            } else {
                format!("v{pkg_version}-{hash}")
            }
        } else {
            // No tags reachable (shallow clone): bare hash like "c6d9bc3"
            format!("v{pkg_version}-{hash}")
        }
    } else {
        // No git info: fallback to Cargo.toml version.
        format!("v{pkg_version}-{hash}")
    };

    println!("cargo:rustc-env=AXE_BUILD_VERSION={version}");
    println!("cargo:rerun-if-env-changed=AXE_NIGHTLY");
}

/// Extracts the base tag (e.g. "v0.1.0") from git describe output,
/// falling back to the Cargo.toml package version.
fn base_version_from_describe(describe: &Option<String>, pkg_version: &str) -> String {
    if let Some(desc) = describe {
        if desc.contains('-') {
            // "v0.1.0-5-gabc123" → "v0.1.0"
            desc.rsplitn(3, '-').last().unwrap_or(desc).to_string()
        } else {
            // Exactly on tag.
            desc.clone()
        }
    } else {
        format!("v{pkg_version}")
    }
}
