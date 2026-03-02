use std::collections::HashMap;
use tower_lsp::lsp_types::*;

use crate::parser::html_document::HtmlDocument;

fn offset_at(text: &str, position: Position) -> usize {
    let mut offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == position.line as usize {
            return offset + (position.character as usize).min(line.len());
        }
        offset += line.len() + 1;
    }
    offset.min(text.len())
}

fn position_at(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

fn is_within_tag_range(node: &crate::parser::html_document::HtmlNode, offset: usize, tag: &str) -> bool {
    if let Some(ets) = node.end_tag_start {
        if ets + 2 <= offset && offset <= ets + 2 + tag.len() {
            return true;
        }
    }
    node.start + 1 <= offset && offset <= node.start + 1 + tag.len()
}

pub fn do_rename(
    text: &str,
    position: Position,
    new_name: &str,
    document: &HtmlDocument,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let offset = offset_at(text, position);
    let node_id = document.find_node_at(offset);
    let node = document.node(node_id);
    let tag = node.tag.as_deref()?;

    if !is_within_tag_range(node, offset, tag) {
        return None;
    }

    let mut edits = Vec::new();

    // Start tag edit
    let start_range = Range::new(
        position_at(text, node.start + 1),
        position_at(text, node.start + 1 + tag.len()),
    );
    edits.push(TextEdit::new(start_range, new_name.to_string()));

    // End tag edit
    if let Some(ets) = node.end_tag_start {
        let end_range = Range::new(
            position_at(text, ets + 2),
            position_at(text, ets + 2 + tag.len()),
        );
        edits.push(TextEdit::new(end_range, new_name.to_string()));
    }

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}
