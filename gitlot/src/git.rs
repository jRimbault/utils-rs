//! Imperative shell over libgit2: walk history, filter by pathspec, list
//! annotated tags. Everything here calls into git2 — no rendering, no
//! merging logic.

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::core::{Commit, Tag};

/// Walk `HEAD` ancestry and keep commits whose diff against the first
/// parent touches at least one path matching `pathspec`. Root commit is
/// diffed against the empty tree.
pub fn path_commits(
    repo: &git2::Repository,
    pathspec: &str,
    head: git2::Oid,
) -> Result<Vec<Commit>> {
    let decorations = build_decoration_map(repo)?;

    let mut walk = repo.revwalk().context("create revwalk")?;
    walk.push(head).context("push HEAD onto revwalk")?;

    let mut out = Vec::new();
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(pathspec);

    for oid in walk {
        let oid = oid.context("walk yielded error")?;
        let commit = repo
            .find_commit(oid)
            .with_context(|| format!("load commit {oid}"))?;
        if commit_touches_path(repo, &commit, &mut diff_opts)? {
            out.push(to_core_commit(&commit, &decorations));
        }
    }
    Ok(out)
}

fn commit_touches_path(
    repo: &git2::Repository,
    commit: &git2::Commit,
    diff_opts: &mut git2::DiffOptions,
) -> Result<bool> {
    let new_tree = commit.tree().context("commit tree")?;
    let parent = if commit.parent_count() > 0 {
        Some(commit.parent(0).context("parent commit")?)
    } else {
        None
    };
    let old_tree = match &parent {
        Some(p) => Some(p.tree().context("parent tree")?),
        None => None,
    };
    let diff = repo
        .diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(diff_opts))
        .context("diff parent..commit with pathspec")?;
    Ok(diff.deltas().len() > 0)
}

fn to_core_commit(commit: &git2::Commit, decorations: &HashMap<git2::Oid, Vec<String>>) -> Commit {
    let author = commit.author();
    Commit {
        id: commit.id(),
        time: commit.time().seconds(),
        author: author.name().unwrap_or("?").to_owned(),
        summary: commit
            .summary()
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_owned(),
        decorations: decorations.get(&commit.id()).cloned().unwrap_or_default(),
    }
}

/// One pass over all references, grouping decorations by commit OID.
/// Cheap and reused for every commit we render.
fn build_decoration_map(repo: &git2::Repository) -> Result<HashMap<git2::Oid, Vec<String>>> {
    let mut map: HashMap<git2::Oid, Vec<String>> = HashMap::new();
    let refs = repo.references().context("iterate references")?;
    for r in refs {
        let Ok(r) = r else {
            continue;
        };
        let Ok(name) = r.shorthand() else {
            continue;
        };
        if let Some(oid) = r.target() {
            map.entry(oid).or_default().push(name.to_owned());
        }
    }
    Ok(map)
}

/// Annotated tags only — lightweight tags are silently skipped.
/// Tag time is the tagger's signature time, falling back to the target
/// commit's author time.
pub fn annotated_tags(repo: &git2::Repository, glob: &str) -> Result<Vec<Tag>> {
    let names = repo
        .tag_names(Some(glob))
        .context("list tag names")?;

    let mut out = Vec::new();
    for entry in &names {
        let Ok(Some(name)) = entry else { continue };
        let refname = format!("refs/tags/{name}");
        let Ok(reference) = repo.find_reference(&refname) else {
            continue;
        };
        let Ok(obj) = reference.peel(git2::ObjectType::Tag) else {
            // Not an annotated tag — skip per design.
            continue;
        };
        let Some(tag) = obj.as_tag() else { continue };
        let target = tag
            .target()
            .with_context(|| format!("peel target of tag {name}"))?
            .peel(git2::ObjectType::Commit)
            .with_context(|| format!("peel commit for tag {name}"))?
            .id();
        let time = match tag.tagger() {
            Some(sig) => sig.when().seconds(),
            None => repo
                .find_commit(target)
                .with_context(|| format!("commit for tag {name}"))?
                .time()
                .seconds(),
        };
        out.push(Tag {
            name: name.to_owned(),
            target,
            time,
        });
    }
    Ok(out)
}
