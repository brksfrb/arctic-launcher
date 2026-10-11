//! "What's new" after an update: release notes split into what's new and what's fixed.
//!
//! Release notes are Markdown written for GitHub (see `docs/release-notes`). A heading line
//! (`### Emotes`, or a line that is only `**Fixed**`) starts a section; sections named like
//! "Fixed" go under *What's fixed*, all others under *What's new*. The download and checksum
//! sections, HTML comments (the update markers) and the "Full Changelog" line are left out.

/// Notes for one or more versions, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleaseNotes {
    pub new: Vec<NoteSection>,
    /// Bullet points (or paragraphs) of the "Fixed" sections.
    pub fixed: Vec<NoteLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSection {
    /// `None` for the text before any sub-heading.
    pub title: Option<String>,
    pub lines: Vec<NoteLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteLine {
    Text(String),
    Bullet(String),
}

/// Sections that are about the release files, not about changes.
const SKIPPED_SECTIONS: [&str; 3] = ["download", "sha-256", "checksums"];

impl ReleaseNotes {
    pub fn is_empty(&self) -> bool {
        self.new.is_empty() && self.fixed.is_empty()
    }

    /// Add another (older) version's notes after these.
    pub fn append(&mut self, older: ReleaseNotes) {
        for section in older.new {
            match self.new.iter_mut().find(|s| s.title == section.title) {
                Some(same) => same.lines.extend(section.lines),
                None => self.new.push(section),
            }
        }
        self.fixed.extend(older.fixed);
    }
}

/// Split one release's Markdown notes.
pub fn parse(markdown: &str) -> ReleaseNotes {
    let mut notes = ReleaseNotes::default();
    let mut current: Option<NoteSection> = Some(NoteSection {
        title: None,
        lines: Vec::new(),
    });
    let mut in_comment = false;
    for raw in markdown.lines() {
        let line = strip_comments(raw, &mut in_comment);
        let line = line.trim();
        if line.is_empty() || line.starts_with("**Full Changelog**") {
            continue;
        }
        if let Some(title) = heading(line) {
            push_section(&mut notes, current.take());
            // "What's new" is the heading over everything: not a section of its own.
            if title.eq_ignore_ascii_case("what's new") {
                current = Some(NoteSection {
                    title: None,
                    lines: Vec::new(),
                });
            } else if !SKIPPED_SECTIONS.contains(&title.to_lowercase().as_str()) {
                current = Some(NoteSection {
                    title: Some(title),
                    lines: Vec::new(),
                });
            }
            continue;
        }
        let Some(section) = current.as_mut() else {
            continue;
        };
        if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            section.lines.push(NoteLine::Bullet(item.trim().to_owned()));
        } else {
            section.lines.push(NoteLine::Text(line.to_owned()));
        }
    }
    push_section(&mut notes, current);
    notes
}

fn push_section(notes: &mut ReleaseNotes, section: Option<NoteSection>) {
    let Some(section) = section.filter(|s| !s.lines.is_empty()) else {
        return;
    };
    let is_fixes = section
        .title
        .as_deref()
        .is_some_and(|t| t.to_lowercase().starts_with("fix"));
    if is_fixes {
        notes.fixed.extend(section.lines);
    } else {
        notes.new.push(section);
    }
}

/// `## Title`, `### Title`, or a line that is only `**Title**`.
fn heading(line: &str) -> Option<String> {
    if line.starts_with('#') {
        let title = line.trim_start_matches('#').trim();
        return (!title.is_empty()).then(|| title.to_owned());
    }
    let inner = line.strip_prefix("**")?.strip_suffix("**")?;
    (!inner.is_empty() && !inner.contains("**")).then(|| inner.trim().to_owned())
}

/// Drop `<!-- … -->` comments, which may span lines.
fn strip_comments(line: &str, in_comment: &mut bool) -> String {
    let mut out = String::new();
    let mut rest = line;
    loop {
        if *in_comment {
            match rest.find("-->") {
                Some(end) => {
                    rest = &rest[end + 3..];
                    *in_comment = false;
                }
                None => return out,
            }
        }
        match rest.find("<!--") {
            Some(start) => {
                out.push_str(&rest[..start]);
                rest = &rest[start + 4..];
                *in_comment = true;
            }
            None => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V034: &str = "## What's new\n\n### Fixed\n- **Motion blur** no longer darkens.\n- Title screen follows the language.\n\n### Download\n\nWindows: `x.exe`\n\n### SHA-256\n\n| File | SHA |\n\n**Full Changelog**: https://x\n";

    #[test]
    fn fixed_section_goes_to_fixed_and_files_are_dropped() {
        let notes = parse(V034);
        assert!(notes.new.is_empty());
        assert_eq!(
            notes.fixed,
            vec![
                NoteLine::Bullet("**Motion blur** no longer darkens.".into()),
                NoteLine::Bullet("Title screen follows the language.".into()),
            ]
        );
    }

    #[test]
    fn intro_markers_and_bold_headings() {
        let md = "## What's new\n\n<!-- arctic:required -->\n**A required update:** downloads fixed.\n\n**Fixed**\n- One\n\n### Emotes\n- 17 new emotes\n";
        let notes = parse(md);
        assert_eq!(
            notes.new,
            vec![
                NoteSection {
                    title: None,
                    lines: vec![NoteLine::Text(
                        "**A required update:** downloads fixed.".into()
                    )],
                },
                NoteSection {
                    title: Some("Emotes".into()),
                    lines: vec![NoteLine::Bullet("17 new emotes".into())],
                },
            ]
        );
        assert_eq!(notes.fixed, vec![NoteLine::Bullet("One".into())]);
    }

    #[test]
    fn multi_line_comments_are_removed() {
        let notes = parse("Before <!-- one\ntwo --> after\n");
        assert_eq!(
            notes.new[0].lines,
            vec![
                NoteLine::Text("Before".into()),
                NoteLine::Text("after".into())
            ]
        );
    }

    #[test]
    fn append_merges_same_titled_sections() {
        let mut newer = parse("### Emotes\n- A\n### Fixed\n- F1\n");
        newer.append(parse(
            "### Emotes\n- B\n### Instances\n- C\n### Fixed\n- F2\n",
        ));
        assert_eq!(newer.new.len(), 2);
        assert_eq!(
            newer.new[0].lines,
            vec![NoteLine::Bullet("A".into()), NoteLine::Bullet("B".into())]
        );
        assert_eq!(newer.fixed.len(), 2);
    }

    #[test]
    fn real_release_notes() {
        let notes = parse(include_str!("../../../../docs/release-notes/v0.3.3.md"));
        let titles: Vec<_> = notes.new.iter().map(|s| s.title.as_deref()).collect();
        assert_eq!(titles[0], None);
        assert!(titles.contains(&Some("Emotes")));
        assert!(!titles.iter().flatten().any(|t| t.contains("Download")));
        assert!(notes.fixed.len() >= 5);
        let text = format!("{notes:?}");
        assert!(!text.contains("arctic:required") && !text.contains("SHA-256"));
    }

    #[test]
    fn empty_notes() {
        assert!(parse("### Download\nfiles\n").is_empty());
    }
}
