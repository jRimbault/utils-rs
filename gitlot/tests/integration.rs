//! End-to-end smoke test against a temp repository.
//!
//! Mirrors the plan's test plan: build a small repo with a tag and a
//! Cargo.toml-only commit, then invoke the binary and assert on its
//! output. Colors are disabled so the assertions are stable.

use std::path::Path;
use std::process::Command;

use git2::{Repository, Signature};

fn sig_at(seconds: i64) -> Signature<'static> {
    Signature::new("Tester", "tester@example.com", &git2::Time::new(seconds, 0)).unwrap()
}

fn write(path: &Path, name: &str, contents: &str) {
    std::fs::write(path.join(name), contents).unwrap();
}

fn commit_all(repo: &Repository, message: &str, seconds: i64) -> git2::Oid {
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = sig_at(seconds);
    let parents: Vec<git2::Commit> = match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
        Some(p) => vec![p],
        None => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .unwrap()
}

fn run_gitlot(repo_path: &Path, args: &[&str]) -> String {
    let exe = env!("CARGO_BIN_EXE_gitlot");
    let output = Command::new(exe)
        .arg("--repo")
        .arg(repo_path)
        .arg("--no-color")
        .args(args)
        .output()
        .expect("run gitlot binary");
    assert!(
        output.status.success(),
        "gitlot failed: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn renders_tag_separator_between_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = Repository::init(tmp.path()).unwrap();

    // First commit: touches src/lib.rs (in path scope).
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    write(&tmp.path().join("src"), "lib.rs", "// v0\n");
    commit_all(&repo, "feat: initial", 1_700_000_000);

    // Second commit: bumps Cargo.toml only — outside src/ scope.
    write(tmp.path(), "Cargo.toml", "[package]\nversion = \"0.1.0\"\n");
    let bump_oid = commit_all(&repo, "chore: release v0.1.0", 1_700_000_100);

    // Annotated tag on the bump commit.
    let tagger = sig_at(1_700_000_100);
    let bump = repo.find_object(bump_oid, None).unwrap();
    repo.tag("v0.1.0", &bump, &tagger, "release v0.1.0", false)
        .unwrap();

    // Third commit in scope.
    write(&tmp.path().join("src"), "lib.rs", "// v1\n");
    commit_all(&repo, "feat: improve lib", 1_700_000_200);

    let out = run_gitlot(tmp.path(), &["src"]);

    assert!(out.contains("v0.1.0"), "tag separator missing: {out}");
    assert!(out.contains("feat: initial"), "initial commit missing: {out}");
    assert!(out.contains("feat: improve lib"), "later commit missing: {out}");
    // The Cargo.toml-only bump must not appear when scoping to src/.
    assert!(
        !out.contains("release v0.1.0"),
        "bump commit leaked into path-scoped output: {out}"
    );

    // Ordering: newer → tag → older.
    let later = out.find("feat: improve lib").unwrap();
    let tag = out.find("v0.1.0").unwrap();
    let initial = out.find("feat: initial").unwrap();
    assert!(later < tag && tag < initial, "wrong order: {out}");
}

#[test]
fn show_bump_commits_keeps_release_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = Repository::init(tmp.path()).unwrap();

    write(tmp.path(), "Cargo.toml", "[package]\nversion = \"0.1.0\"\n");
    let bump_oid = commit_all(&repo, "chore: release v0.1.0", 1_700_000_100);

    let tagger = sig_at(1_700_000_100);
    let bump = repo.find_object(bump_oid, None).unwrap();
    repo.tag("v0.1.0", &bump, &tagger, "release v0.1.0", false)
        .unwrap();

    let out = run_gitlot(tmp.path(), &["Cargo.toml", "--show-bump-commits"]);
    assert!(out.contains("release v0.1.0"), "bump commit should be shown: {out}");
    assert!(out.contains("v0.1.0"), "tag separator should be shown: {out}");
}
