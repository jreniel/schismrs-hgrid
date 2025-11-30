// build.rs
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let crate_env_name = "SCHISMRS_HGRID_VERSION";
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    let gitrepo_path = ".gitrepo";

    let version_suffix = if Path::new(gitrepo_path).exists() {
        // We're in a git subrepo - get both workspace and local info

        // Get current workspace HEAD (what the workspace is at)
        let workspace_hash = std::process::Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .output()
            .map(|output| {
                let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if hash.len() >= 8 {
                    hash[..8].to_string()
                } else {
                    hash
                }
            })
            .unwrap_or_else(|_| "unknown".to_string());

        // Check if workspace (excluding current directory) is dirty
        let workspace_dirty = std::process::Command::new("git")
            .args(&["diff", "--quiet", ":(exclude)."])
            .status()
            .map(|status| if status.success() { "" } else { "-dirty" })
            .unwrap_or("");

        // Get local subrepo commit from .gitrepo file
        let gitrepo_content = fs::read_to_string(gitrepo_path)?;
        let local_commit =
            parse_subrepo_commit(&gitrepo_content).unwrap_or_else(|| "unknown".to_string());

        let local_hash = if local_commit.len() >= 8 {
            local_commit[..8].to_string()
        } else {
            local_commit
        };

        // Check if local directory is dirty
        let local_dirty = std::process::Command::new("git")
            .args(&["diff", "--quiet", "."])
            .status()
            .map(|status| if status.success() { "" } else { "-dirty" })
            .unwrap_or("");

        format!(
            "{}{}-{}{}-{}",
            workspace_hash, workspace_dirty, local_hash, local_dirty, profile
        )
    } else {
        // Standalone mode - just local hash and dirty state
        let local_hash = std::process::Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .output()
            .map(|output| {
                let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if hash.len() >= 8 {
                    hash[..8].to_string()
                } else {
                    hash
                }
            })
            .unwrap_or_else(|_| "unknown".to_string());

        let local_dirty = std::process::Command::new("git")
            .args(&["diff", "--quiet"])
            .status()
            .map(|status| if status.success() { "" } else { "-dirty" })
            .unwrap_or("");

        format!("{}{}-{}", local_hash, local_dirty, profile)
    };

    let full_version = format!("{} {}", version, version_suffix);

    println!("cargo:rustc-env={}={}", crate_env_name, full_version);

    // Tell cargo to rerun if relevant files change
    println!("cargo:rerun-if-changed=.gitrepo");
    println!("cargo:rerun-if-changed=.git/HEAD");

    Ok(())
}

fn parse_subrepo_commit(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("commit = ") {
            return Some(line.strip_prefix("commit = ")?.to_string());
        }
    }
    None
}
