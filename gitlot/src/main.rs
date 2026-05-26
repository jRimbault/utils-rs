//! gitlot — `git log <path>` with annotated-tag separators interleaved.
//!
//! See PLAN.md for the rationale. The shell here is the imperative layer:
//! CLI parsing, pager wiring, rendering. All sorting / interleaving lives
//! in `core.rs`; all libgit2 calls live in `git.rs`.

mod core;
mod git;

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::Parser;
use console::style;

use crate::core::{Entry, MergeOpts};

#[derive(Parser, Debug)]
#[command(
    about = "git log <path> with annotated-tag separator lines interleaved",
    version
)]
struct Cli {
    /// Pathspec to scope the log to.
    #[arg(value_name = "PATH")]
    path: String,

    /// Repository root. Defaults to discovery from the current directory.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// Glob filter for tag names.
    #[arg(long, value_name = "GLOB", default_value = "v*")]
    tag_glob: String,

    /// Disable ANSI colors.
    #[arg(long)]
    no_color: bool,

    /// Maximum number of commits to display.
    #[arg(long, value_name = "N")]
    limit: Option<usize>,

    /// Show release-bump commits in addition to the tag separator.
    #[arg(long)]
    show_bump_commits: bool,
}

#[derive(Debug)]
struct Args {
    path: String,
    repo: Option<PathBuf>,
    tag_glob: String,
    no_color: bool,
    merge: MergeOpts,
}

impl Args {
    fn from_cli(cli: Cli) -> Self {
        Self {
            path: cli.path,
            repo: cli.repo,
            tag_glob: cli.tag_glob,
            no_color: cli.no_color,
            merge: MergeOpts {
                show_bump_commits: cli.show_bump_commits,
                limit: cli.limit,
            },
        }
    }
}

fn parse_args() -> Args {
    Args::from_cli(Cli::parse())
}

fn main() -> Result<()> {
    let args = parse_args();

    let repo = open_repo(args.repo.as_deref())?;
    let head = head_commit_id(&repo)?;

    let commits = git::path_commits(&repo, &args.path, head)?;
    let tags = git::annotated_tags(&repo, &args.tag_glob, head)?;
    let entries = core::merge(commits, tags, args.merge);

    let mut pager = Pager::spawn();
    // Piping through the pager defeats `console`'s tty detection. Force
    // colors back on unless the user explicitly disabled them.
    if args.no_color {
        console::set_colors_enabled(false);
    } else if pager.child.is_some() {
        console::set_colors_enabled(true);
    }

    let mut out = pager.writer();
    let now = jiff::Timestamp::now();
    for e in &entries {
        if let Err(err) = render_entry(&mut out, e, now) {
            if err.kind() == std::io::ErrorKind::BrokenPipe {
                break;
            }
            return Err(err.into());
        }
    }
    drop(out);
    Ok(())
}

fn open_repo(explicit: Option<&std::path::Path>) -> Result<git2::Repository> {
    match explicit {
        Some(p) => git2::Repository::open(p).with_context(|| format!("open repo at {p:?}")),
        None => {
            let cwd = std::env::current_dir().context("read current directory")?;
            git2::Repository::discover(&cwd).with_context(|| format!("discover repo from {cwd:?}"))
        }
    }
}

fn head_commit_id(repo: &git2::Repository) -> Result<git2::Oid> {
    match repo.head() {
        Ok(head) => head
            .peel_to_commit()
            .context("peel HEAD to commit")
            .map(|commit| commit.id()),
        Err(err) if err.code() == git2::ErrorCode::UnbornBranch => {
            bail!("repository has no commits yet; create an initial commit before running gitlot")
        }
        Err(err) => Err(err).context("read HEAD"),
    }
}

fn render_entry(out: &mut dyn Write, e: &Entry, now: jiff::Timestamp) -> std::io::Result<()> {
    match e {
        Entry::Commit(c) => {
            let full = c.id.to_string();
            let short = &full[..full.len().min(7)];
            let rel = relative_time(c.time, now);
            let decoration = if c.decorations.is_empty() {
                String::new()
            } else {
                format!(" ({})", c.decorations.join(", "))
            };
            writeln!(
                out,
                "{} {} {} {}{}",
                style(short).red(),
                style(rel).green(),
                style(&c.author).blue(),
                c.summary,
                style(decoration).yellow(),
            )
        }
        Entry::TagSeparator { name, time } => {
            let rel = relative_time(*time, now);
            let bar = "━━━";
            writeln!(
                out,
                "{bar} {} {} {} {bar}",
                style("tag").dim(),
                style(name).yellow().bold(),
                style(format!("— {rel}")).dim(),
            )
        }
    }
}

fn relative_time(unix_seconds: i64, now: jiff::Timestamp) -> String {
    let Ok(ts) = jiff::Timestamp::from_second(unix_seconds) else {
        return "?".into();
    };
    let diff = now.as_second().saturating_sub(ts.as_second());
    if diff < 0 {
        return "in the future".into();
    }
    let (n, unit) = match diff {
        s if s < 60 => (s, "s"),
        s if s < 3_600 => (s / 60, "m"),
        s if s < 86_400 => (s / 3_600, "h"),
        s if s < 86_400 * 30 => (s / 86_400, "d"),
        s if s < 86_400 * 365 => (s / (86_400 * 30), "mo"),
        s => (s / (86_400 * 365), "y"),
    };
    format!("{n}{unit} ago")
}

/// Stream output through `$PAGER` when stdout is a tty.
struct Pager {
    child: Option<Child>,
}

impl Pager {
    fn spawn() -> Self {
        if !std::io::stdout().is_terminal() {
            return Self { child: None };
        }
        let cmd = std::env::var("PAGER").unwrap_or_else(|_| "less -FRX".into());
        let child = Command::new("sh")
            .args(["-c", &cmd])
            .stdin(Stdio::piped())
            .spawn()
            .ok();
        Self { child }
    }

    fn writer(&mut self) -> Box<dyn Write + '_> {
        match self.child.as_mut().and_then(|c| c.stdin.as_mut()) {
            Some(stdin) => Box::new(stdin),
            None => Box::new(std::io::stdout().lock()),
        }
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            drop(c.stdin.take());
            let _ = c.wait();
        }
    }
}
