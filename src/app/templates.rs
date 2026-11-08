//! Starter documents offered from the empty preview.

use egui_phosphor::regular as icons;

pub struct Template {
    pub name: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        name: "README",
        icon: icons::BOOK_OPEN_TEXT,
        description: "Project overview with features, setup and license sections",
        body: r#"# Project name

One sentence that explains what this project does.

## Features

- Feature one
- Feature two
- Feature three

## Getting started

```bash
git clone https://github.com/you/project.git
cd project
cargo run
```

## Usage

Describe the main workflow here.

## Contributing

Pull requests are welcome. Open an issue first for larger changes.

## License

Add your license here.
"#,
    },
    Template {
        name: "Meeting notes",
        icon: icons::USERS_THREE,
        description: "Agenda, decisions and action items",
        body: r#"# Meeting notes — YYYY-MM-DD

**Attendees:** Name, Name

## Agenda

- Topic one
- Topic two

## Notes

- ...

## Decisions

- ...

## Action items

- [ ] Owner — task (due date)
- [ ] Owner — task (due date)
"#,
    },
    Template {
        name: "Blog post",
        icon: icons::ARTICLE,
        description: "Title, intro, sections and a conclusion",
        body: r#"# Post title

*Published YYYY-MM-DD · 5 min read*

A short opening paragraph that hooks the reader.

## Section heading

Write the main idea here, with **bold** and *italic* emphasis where useful.

> A pull quote or takeaway.

## Another section

- Point one
- Point two

## Conclusion

Wrap up with the key takeaway and a call to action.
"#,
    },
    Template {
        name: "Changelog",
        icon: icons::GIT_COMMIT,
        description: "Keep a release history",
        body: r#"# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Added

- ...

### Changed

- ...

### Fixed

- ...

## [0.1.0] — YYYY-MM-DD

### Added

- Initial release.
"#,
    },
    Template {
        name: "Task list",
        icon: icons::CHECK_SQUARE,
        description: "A quick checklist split into today and later",
        body: r#"# Tasks

## Today

- [ ] First task
- [ ] Second task

## Later

- [ ] Backlog item
- [ ] Backlog item
"#,
    },
    Template {
        name: "Project brief",
        icon: icons::TARGET,
        description: "Problem, goals and scope",
        body: r#"# Project brief

## Problem

What problem does this solve, and for whom?

## Goals

- Goal one
- Goal two

## Non-goals

- ...

## Scope

### In scope

- ...

### Out of scope

- ...

## Milestones

| Milestone | Date | Status |
| --- | --- | --- |
| Kickoff | YYYY-MM-DD | Planned |
| Beta | YYYY-MM-DD | Planned |
| Launch | YYYY-MM-DD | Planned |
"#,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_is_named_and_documented() {
        assert!(TEMPLATES.len() >= 4);
        for template in TEMPLATES {
            assert!(!template.name.trim().is_empty());
            assert!(!template.description.trim().is_empty());
            assert!(template.body.starts_with("# "), "{}", template.name);
            assert!(template.body.ends_with('\n'), "{}", template.name);
        }
    }

    #[test]
    fn templates_have_distinct_names() {
        let mut names: Vec<_> = TEMPLATES.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), TEMPLATES.len());
    }
}
