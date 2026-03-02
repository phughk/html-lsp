use tower_lsp::lsp_types::*;

use crate::parser::html_document::HtmlDocument;

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

fn node_to_name(doc: &HtmlDocument, node_id: usize) -> String {
    let node = doc.node(node_id);
    let mut name = node.tag.clone().unwrap_or_else(|| "?".to_string());

    if let Some(id) = node.attributes.get("id") {
        if let Some(id_val) = id {
            let id_clean = id_val.trim_matches(|c| c == '"' || c == '\'');
            name.push('#');
            name.push_str(id_clean);
        }
    }

    if let Some(classes) = node.attributes.get("class") {
        if let Some(class_val) = classes {
            let class_clean = class_val.trim_matches(|c| c == '"' || c == '\'');
            for cls in class_clean.split_whitespace() {
                name.push('.');
                name.push_str(cls);
            }
        }
    }

    name
}

#[allow(deprecated)]
pub fn find_document_symbols(
    text: &str,
    document: &HtmlDocument,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for &root_id in &document.roots {
        if let Some(sym) = build_symbol(text, document, root_id) {
            symbols.push(sym);
        }
    }
    symbols
}

#[allow(deprecated)]
fn build_symbol(
    text: &str,
    document: &HtmlDocument,
    node_id: usize,
) -> Option<DocumentSymbol> {
    let node = document.node(node_id);
    let name = node_to_name(document, node_id);
    let range = Range::new(position_at(text, node.start), position_at(text, node.end));

    let mut children_symbols = Vec::new();
    for &child_id in &node.children {
        if let Some(child_sym) = build_symbol(text, document, child_id) {
            children_symbols.push(child_sym);
        }
    }

    Some(DocumentSymbol {
        name,
        detail: None,
        kind: SymbolKind::FIELD,
        tags: None,
        deprecated: None,
        range,
        selection_range: range,
        children: if children_symbols.is_empty() {
            None
        } else {
            Some(children_symbols)
        },
    })
}
