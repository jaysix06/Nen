use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Numbered,
    Checklist,
}

struct Prefix<'a> {
    indent: &'a str,
    marker: &'a str,
    content: &'a str,
    number: Option<u32>,
    kind: ListKind,
}

#[derive(Clone, Debug)]
pub struct ChecklistItem {
    pub marker: Range<usize>,
    pub checked: bool,
    pub label: String,
    pub indent: usize,
}

fn checkbox(line: &str) -> Option<ChecklistItem> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent = line.len() - trimmed.len();
    let text = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
        .unwrap_or(trimmed);
    let marker_start = line.len() - text.len();
    if !(text.starts_with("[ ]") || text.starts_with("[x]") || text.starts_with("[X]"))
        || !text[3..].starts_with([' ', '\t']) && text.len() != 3
    {
        return None;
    }
    Some(ChecklistItem {
        marker: indent..marker_start + 3,
        checked: text.as_bytes()[1] != b' ',
        label: text[3..].trim_start_matches([' ', '\t']).to_owned(),
        indent,
    })
}

/// Source ranges stay in bytes, including any legacy bullet before the box.
pub fn checklist_items(text: &str) -> Vec<ChecklistItem> {
    let mut items = Vec::new();
    let mut offset = 0;
    let mut fence = None;
    for line in text.split_inclusive('\n') {
        let line_text = line.trim_end_matches(['\r', '\n']);
        let trimmed = line_text.trim_start();
        let next = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };
        if let Some(next) = next {
            if fence == Some(next) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(next);
            }
        } else if fence.is_none()
            && let Some(mut item) = checkbox(line_text)
        {
            item.marker.start += offset;
            item.marker.end += offset;
            items.push(item);
        }
        offset += line.len();
    }
    items
}

fn prefix(line: &str) -> Option<Prefix<'_>> {
    if let Some(item) = checkbox(line) {
        return Some(Prefix {
            indent: &line[..item.indent],
            marker: &line[item.marker.clone()],
            content: line[item.marker.end..].trim_start_matches([' ', '\t']),
            number: None,
            kind: ListKind::Checklist,
        });
    }
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent = &line[..line.len() - trimmed.len()];
    let split = trimmed.find([' ', '\t'])?;
    let marker = &trimmed[..split];
    let number = marker.strip_suffix(['.', ')']).and_then(|digits| {
        (digits.len() <= 9 && digits.bytes().all(|c| c.is_ascii_digit()))
            .then(|| digits.parse::<u32>().ok())
            .flatten()
    });
    if number.is_none() && !matches!(marker, "-" | "+" | "*" | "•" | "." | "–" | "—") {
        return None;
    }
    let mut content = trimmed[split..].trim_start_matches([' ', '\t']);
    let mut kind = if number.is_some() {
        ListKind::Numbered
    } else {
        ListKind::Bullet
    };
    if number.is_none()
        && (content.starts_with("[ ] ")
            || content.starts_with("[x] ")
            || content.starts_with("[X] ")
            || matches!(content, "[ ]" | "[x]" | "[X]"))
    {
        content = content[3..].trim_start_matches([' ', '\t']);
        kind = ListKind::Checklist;
    }
    Some(Prefix {
        indent,
        marker,
        content,
        number,
        kind,
    })
}

fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |index| index + 1)
}

fn in_code(text: &str, offset: usize) -> bool {
    let mut fence = None;
    for line in text[..line_start(text, offset)].lines() {
        let line = line.trim_start();
        let next = if line.starts_with("```") {
            Some('`')
        } else if line.starts_with("~~~") {
            Some('~')
        } else {
            None
        };
        if let Some(next) = next {
            if fence == Some(next) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(next);
            }
        }
    }
    fence.is_some()
}

/// Complete a marker as it is typed, and continue or end lists on Enter.
pub fn typing(text: &str, selection: Range<usize>, key: &str) -> Option<(Range<usize>, String)> {
    if !selection.is_empty() || in_code(text, selection.start) {
        return None;
    }
    let start = line_start(text, selection.start);
    let before = &text[start..selection.start];
    if key == "space" {
        let candidate = format!("{before} ");
        let parsed = prefix(&candidate)?;
        if parsed.kind == ListKind::Checklist {
            return None;
        }
        if !parsed.content.is_empty() {
            return None;
        }
        let marker = if parsed.number.is_some() {
            parsed.marker
        } else {
            "-"
        };
        return Some((
            start..selection.start,
            format!("{}{marker} ", parsed.indent),
        ));
    }
    let parsed = prefix(before)?;
    if key == "enter" {
        let end = text[selection.end..]
            .find('\n')
            .map_or(text.len(), |offset| selection.end + offset);
        if parsed.content.is_empty() && text[selection.end..end].trim().is_empty() {
            return Some((start..selection.end, parsed.indent.to_owned()));
        }
        let marker = match parsed.kind {
            ListKind::Numbered => format!(
                "{}{}",
                parsed.number?.checked_add(1)?,
                parsed.marker.chars().last()?
            ),
            ListKind::Checklist => "[ ]".into(),
            ListKind::Bullet => "-".into(),
        };
        return Some((selection, format!("\n{}{marker} ", parsed.indent)));
    }
    if key == "tab" {
        return Some((start..start, "    ".into()));
    }
    if key == "shift-tab" {
        let count = if parsed.indent.starts_with('\t') {
            1
        } else {
            parsed.indent.len().min(4)
        };
        return Some((start..start + count, String::new()));
    }
    None
}

pub fn toggle(text: &str, selection: Range<usize>, kind: ListKind) -> (Range<usize>, String) {
    let start = line_start(text, selection.start);
    let selection_end = if selection.end > selection.start && text[..selection.end].ends_with('\n')
    {
        selection.end - 1
    } else {
        selection.end
    };
    let end = text[selection_end..]
        .find('\n')
        .map_or(text.len(), |offset| selection_end + offset);
    let lines: Vec<_> = text[start..end].split('\n').collect();
    let remove = lines
        .iter()
        .all(|line| prefix(line).is_some_and(|p| p.kind == kind));
    let result = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let (indent, content) = if let Some(p) = prefix(line) {
                (p.indent, p.content)
            } else {
                let content = line.trim_start_matches([' ', '\t']);
                (&line[..line.len() - content.len()], content)
            };
            if remove {
                return format!("{indent}{content}");
            }
            let marker = match kind {
                ListKind::Bullet => "-".into(),
                ListKind::Numbered => format!("{}.", index + 1),
                ListKind::Checklist => "[ ]".into(),
            };
            format!("{indent}{marker} {content}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    (start..end, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lists_continue_exit_indent_and_leave_code_and_decimals_alone() {
        for (text, expected) in [
            ("1. first", "\n2. "),
            ("  - item", "\n  - "),
            ("- [x] done", "\n[ ] "),
            ("[x] done", "\n[ ] "),
            ("12) item", "\n13) "),
            ("• item", "\n- "),
        ] {
            assert_eq!(
                typing(text, text.len()..text.len(), "enter").unwrap().1,
                expected
            );
        }
        assert_eq!(typing("  - ", 4..4, "enter"), Some((0..4, "  ".into())));
        assert_eq!(typing("•", 3..3, "space"), Some((0..3, "- ".into())));
        assert!(typing("1.5", 3..3, "space").is_none());
        let code = "```\n- item";
        assert!(typing(code, code.len()..code.len(), "enter").is_none());
        assert_eq!(typing("    - item", 10..10, "shift-tab").unwrap().0, 0..4);
    }
    #[test]
    fn toolbar_formats_selected_lines_and_toggles_existing_lists() {
        let text = "before\n猫\n  two\nafter";
        let (range, numbered) = toggle(text, 7..16, ListKind::Numbered);
        assert_eq!(range, 7..16);
        assert_eq!(numbered, "1. 猫\n  2. two");
        assert_eq!(
            toggle(&numbered, 0..numbered.len(), ListKind::Numbered).1,
            "猫\n  two"
        );
        assert_eq!(
            toggle("one\ntwo", 0..7, ListKind::Checklist).1,
            "[ ] one\n[ ] two"
        );
        assert_eq!(typing("[ ] ", 4..4, "enter"), Some((0..4, String::new())));
        let text = "[ ] 猫\n- [x] old\n```\n[ ] code\n```";
        let items = checklist_items(text);
        assert_eq!(items.len(), 2);
        assert_eq!(&text[items[0].marker.clone()], "[ ]");
        assert_eq!(&text[items[1].marker.clone()], "- [x]");
        assert!(items[1].checked);
    }
}
