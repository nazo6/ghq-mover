use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use git2::Repository;
use walkdir::WalkDir;

fn main() {
    if let Err(err) = main_inner() {
        eprintln!("Error: {:?}", err);
        std::process::exit(1);
    }
}

fn main_inner() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <directory>", args[0]);
        std::process::exit(1);
    }

    let target_dir = PathBuf::from(&args[1]).canonicalize()?;
    println!("🔍 Searching for Git repositories in {:?}", target_dir);

    let repos = find_git_repos(&target_dir)?;

    if repos.is_empty() {
        println!("⚠️  No Git repositories found.");
        return Ok(());
    }

    println!("\n✅ Found repositories:");
    for r in &repos {
        println!("- {:?}\n  → {:?}\n", r.0, r.1);
    }

    if !confirm("Do you want to move these repositories to ~/ghq? [y/N]: ")? {
        println!("🚫 Operation cancelled.");
        return Ok(());
    }

    for (src, dest) in repos {
        println!("🚚 Moving {:?} → {:?}", src, dest);

        if dest.exists() {
            println!("⚠️  Destination {:?} already exists. Skipping.", dest);
            continue;
        }
        if let Err(e) = fs::create_dir_all(dest.parent().unwrap()) {
            println!(
                "⚠️  Failed to create parent directory for {:?}: {:?}",
                dest, e
            );
            continue;
        }
        if let Err(e) = fs::rename(&src, &dest)
            .with_context(|| format!("Failed to move {:?} to {:?}", src, dest))
        {
            println!("⚠️  Failed to move directory {:?}", e);
        }
    }

    println!("🎉 Done!");
    Ok(())
}

/// 確認用プロンプト
fn confirm(prompt: &str) -> Result<bool> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn find_git_repos(base: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut repos = Vec::new();

    let ghq_dir = if let Ok(ghq_root) = std::env::var("GHQ_ROOT") {
        PathBuf::from(ghq_root)
    } else {
        let mut ghq_dir = std::env::home_dir().context("Failed to find home directory")?;
        ghq_dir.push("ghq");
        ghq_dir
    };

    let mut it = WalkDir::new(base)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir());
    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => continue,
            Some(Ok(entry)) => entry,
        };
        if entry.file_type().is_dir() && entry.file_name() == ".git" {
            it.skip_current_dir();

            let git_dir = entry.path();
            let repo_root = git_dir.parent().unwrap_or(git_dir);
            if let Ok(repo) = Repository::open(repo_root)
                && let Ok(remote) = repo.find_remote("origin")
                && let Some(url) = remote.url()
            {
                let Ok(git_url) = git_url_parse::GitUrl::parse(url) else {
                    continue;
                };
                let Some((owner, repo)) = git_url.path().split_once('/') else {
                    continue;
                };

                let mut target_path = ghq_dir.clone();
                target_path.push(git_url.host().context("Invalid Git URL")?);
                target_path.push(owner);
                target_path.push(repo.trim_end_matches(".git"));

                repos.push((repo_root.to_path_buf(), target_path));
            }
        }
    }

    Ok(repos)
}
