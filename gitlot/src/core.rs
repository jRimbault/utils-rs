//! Pure merging of path-scoped commits with annotated tag separators.
//!
//! No I/O, no git2 calls past type re-exports — everything here is
//! deterministic and trivially testable.

use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: git2::Oid,
    pub time: i64,
    pub author: String,
    pub summary: String,
    pub decorations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub target: git2::Oid,
    pub time: i64,
}

#[derive(Debug, Clone)]
pub enum Entry {
    Commit(Commit),
    TagSeparator { name: String, time: i64 },
}

#[derive(Debug, Clone, Copy)]
pub struct MergeOpts {
    pub show_bump_commits: bool,
    pub limit: Option<usize>,
}

/// Interleave `commits` and `tags` into a single descending-time stream.
///
/// Tag-bump commits (commits an annotated tag points at) are dropped by
/// default — the separator already marks that point in history. Set
/// `show_bump_commits` to keep them.
pub fn merge(commits: Vec<Commit>, tags: Vec<Tag>, opts: MergeOpts) -> Vec<Entry> {
    // OID -> count of tags landing on that commit. Used both to decide
    // whether a path commit is a "bump commit" and to emit one separator
    // per tag even when several tags share an OID.
    let mut tags_by_oid: HashMap<git2::Oid, usize> = HashMap::new();
    for t in &tags {
        *tags_by_oid.entry(t.target).or_insert(0) += 1;
    }

    let mut entries: Vec<Entry> = Vec::with_capacity(commits.len() + tags.len());

    for t in tags {
        entries.push(Entry::TagSeparator {
            name: t.name,
            time: t.time,
        });
    }

    for c in commits {
        if !opts.show_bump_commits && tags_by_oid.contains_key(&c.id) {
            continue;
        }
        entries.push(Entry::Commit(c));
    }

    // Descending by time. Tiebreaks:
    // - Separators sort before commits at equal time (they introduce the
    //   commits below them).
    // - Two tags at the same time sort by descending semver when both
    //   names parse as versions, then fall back to descending name order.
    // - Two commits at the same time fall back to ascending OID for
    //   deterministic test output.
    entries.sort_by(|a, b| {
        let (ta, tb) = (entry_time(a), entry_time(b));
        tb.cmp(&ta)
            .then_with(|| entry_kind_rank(a).cmp(&entry_kind_rank(b)))
            .then_with(|| match (a, b) {
                (Entry::TagSeparator { name: n1, .. }, Entry::TagSeparator { name: n2, .. }) => {
                    compare_tag_names(n1, n2)
                }
                (Entry::Commit(c1), Entry::Commit(c2)) => c1.id.cmp(&c2.id),
                _ => std::cmp::Ordering::Equal,
            })
    });

    if let Some(limit) = opts.limit {
        let mut kept = Vec::with_capacity(entries.len());
        let mut commits_emitted = 0usize;
        for e in entries {
            if matches!(e, Entry::Commit(_)) {
                if commits_emitted >= limit {
                    continue;
                }
                commits_emitted += 1;
            }
            kept.push(e);
        }
        // Trim trailing separators that no longer precede a commit.
        while matches!(kept.last(), Some(Entry::TagSeparator { .. })) {
            kept.pop();
        }
        kept
    } else {
        entries
    }
}

fn compare_tag_names(left: &str, right: &str) -> Ordering {
    match (parse_semver(left), parse_semver(right)) {
        (Some(v1), Some(v2)) => v2.cmp(&v1).then_with(|| right.cmp(left)),
        _ => right.cmp(left),
    }
}

fn parse_semver(name: &str) -> Option<semver::Version> {
    semver::Version::parse(name).ok().or_else(|| {
        name.strip_prefix('v')
            .and_then(|rest| semver::Version::parse(rest).ok())
    })
}

fn entry_time(e: &Entry) -> i64 {
    match e {
        Entry::Commit(c) => c.time,
        Entry::TagSeparator { time, .. } => *time,
    }
}

fn entry_kind_rank(e: &Entry) -> u8 {
    match e {
        Entry::TagSeparator { .. } => 0,
        Entry::Commit(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(byte: u8) -> git2::Oid {
        let mut buf = [0u8; 20];
        buf[19] = byte;
        git2::Oid::from_bytes(&buf).unwrap()
    }

    fn commit(byte: u8, time: i64, summary: &str) -> Commit {
        Commit {
            id: oid(byte),
            time,
            author: "a".into(),
            summary: summary.into(),
            decorations: vec![],
        }
    }

    fn tag(name: &str, target: u8, time: i64) -> Tag {
        Tag {
            name: name.into(),
            target: oid(target),
            time,
        }
    }

    fn opts(show_bumps: bool, limit: Option<usize>) -> MergeOpts {
        MergeOpts {
            show_bump_commits: show_bumps,
            limit,
        }
    }

    #[test]
    fn empty_inputs() {
        assert!(merge(vec![], vec![], opts(false, None)).is_empty());
    }

    #[test]
    fn tag_not_in_path_set_still_emits_separator() {
        let entries = merge(
            vec![commit(1, 100, "a"), commit(2, 50, "b")],
            vec![tag("v1", 9, 80)],
            opts(false, None),
        );
        assert_eq!(entries.len(), 3);
        assert!(matches!(&entries[0], Entry::Commit(c) if c.summary == "a"));
        assert!(matches!(&entries[1], Entry::TagSeparator { name, .. } if name == "v1"));
        assert!(matches!(&entries[2], Entry::Commit(c) if c.summary == "b"));
    }

    #[test]
    fn tag_on_path_commit_hides_bump_by_default() {
        let entries = merge(
            vec![commit(1, 100, "feat"), commit(2, 50, "release v1")],
            vec![tag("v1", 2, 50)],
            opts(false, None),
        );
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0], Entry::Commit(c) if c.summary == "feat"));
        assert!(matches!(&entries[1], Entry::TagSeparator { name, .. } if name == "v1"));
    }

    #[test]
    fn show_bump_commits_keeps_both() {
        let entries = merge(
            vec![commit(2, 50, "release v1")],
            vec![tag("v1", 2, 50)],
            opts(true, None),
        );
        assert_eq!(entries.len(), 2);
        // Separator first at equal time.
        assert!(matches!(&entries[0], Entry::TagSeparator { .. }));
        assert!(matches!(&entries[1], Entry::Commit(_)));
    }

    #[test]
    fn multiple_tags_between_two_commits() {
        let entries = merge(
            vec![commit(1, 100, "newer"), commit(2, 10, "older")],
            vec![tag("v1", 9, 80), tag("v2", 9, 50)],
            opts(false, None),
        );
        let names: Vec<_> = entries
            .iter()
            .map(|e| match e {
                Entry::Commit(c) => format!("c:{}", c.summary),
                Entry::TagSeparator { name, .. } => format!("t:{name}"),
            })
            .collect();
        assert_eq!(names, vec!["c:newer", "t:v1", "t:v2", "c:older"]);
    }

    #[test]
    fn equal_time_tags_sort_by_semver() {
        let entries = merge(
            vec![],
            vec![tag("v1.9.0", 9, 100), tag("v1.10.0", 9, 100)],
            opts(false, None),
        );
        let names: Vec<_> = entries
            .iter()
            .map(|e| match e {
                Entry::TagSeparator { name, .. } => name.as_str(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(names, vec!["v1.10.0", "v1.9.0"]);
    }

    #[test]
    fn equal_time_non_semver_tags_fall_back_to_name() {
        let entries = merge(
            vec![],
            vec![tag("release-a", 9, 100), tag("release-b", 9, 100)],
            opts(false, None),
        );
        let names: Vec<_> = entries
            .iter()
            .map(|e| match e {
                Entry::TagSeparator { name, .. } => name.as_str(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(names, vec!["release-b", "release-a"]);
    }

    #[test]
    fn identical_timestamps_stable_by_oid() {
        let entries = merge(
            vec![commit(2, 100, "b"), commit(1, 100, "a")],
            vec![],
            opts(false, None),
        );
        assert!(matches!(&entries[0], Entry::Commit(c) if c.summary == "a"));
        assert!(matches!(&entries[1], Entry::Commit(c) if c.summary == "b"));
    }

    #[test]
    fn limit_truncates_commits_only() {
        let entries = merge(
            vec![commit(1, 100, "a"), commit(2, 80, "b"), commit(3, 60, "c")],
            vec![tag("v1", 9, 90)],
            opts(false, Some(2)),
        );
        let commit_count = entries
            .iter()
            .filter(|e| matches!(e, Entry::Commit(_)))
            .count();
        assert_eq!(commit_count, 2);
        // The separator at t=90 sits between the two kept commits.
        assert!(
            entries
                .iter()
                .any(|e| matches!(e, Entry::TagSeparator { .. }))
        );
    }

    #[test]
    fn limit_drops_trailing_separators() {
        let entries = merge(
            vec![commit(1, 100, "a"), commit(2, 50, "b")],
            vec![tag("v1", 9, 10)],
            opts(false, Some(1)),
        );
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0], Entry::Commit(c) if c.summary == "a"));
    }
}
