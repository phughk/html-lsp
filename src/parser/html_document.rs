use std::collections::HashMap;

use super::scanner::{create_scanner, ScannerState, TokenType};

#[derive(Debug, Clone)]
pub struct HtmlNode {
    pub tag: Option<String>,
    pub closed: bool,
    pub start: usize,
    pub end: usize,
    pub start_tag_end: Option<usize>,
    pub end_tag_start: Option<usize>,
    pub attributes: HashMap<String, Option<String>>,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
}

impl HtmlNode {
    fn new(start: usize, end: usize, parent: Option<usize>) -> Self {
        Self {
            tag: None,
            closed: false,
            start,
            end,
            start_tag_end: None,
            end_tag_start: None,
            attributes: HashMap::new(),
            children: Vec::new(),
            parent,
        }
    }

    pub fn is_same_tag(&self, tag_lower: Option<&str>) -> bool {
        match (&self.tag, tag_lower) {
            (None, None) => true,
            (Some(t), Some(other)) => t.to_lowercase() == other,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct HtmlDocument {
    pub nodes: Vec<HtmlNode>,
    pub roots: Vec<usize>,
}

impl HtmlDocument {
    pub fn find_node_before(&self, offset: usize) -> usize {
        self.find_node_before_in(offset, &self.roots, None)
    }

    fn find_node_before_in(&self, offset: usize, children: &[usize], parent: Option<usize>) -> usize {
        let idx = children.partition_point(|&id| self.nodes[id].start < offset);
        if idx > 0 {
            let child_id = children[idx - 1];
            let child = &self.nodes[child_id];
            if offset > child.start {
                if offset < child.end {
                    return self.find_node_before_in(offset, &child.children, Some(child_id));
                }
                if let Some(&last) = child.children.last() {
                    if self.nodes[last].end == child.end {
                        return self.find_node_before_in(offset, &child.children, Some(child_id));
                    }
                }
                return child_id;
            }
        }
        parent.unwrap_or(0)
    }

    pub fn find_node_at(&self, offset: usize) -> usize {
        self.find_node_at_in(offset, &self.roots, None)
    }

    fn find_node_at_in(&self, offset: usize, children: &[usize], parent: Option<usize>) -> usize {
        let idx = children.partition_point(|&id| self.nodes[id].start < offset);
        if idx > 0 {
            let child_id = children[idx - 1];
            let child = &self.nodes[child_id];
            if offset > child.start && offset <= child.end {
                return self.find_node_at_in(offset, &child.children, Some(child_id));
            }
        }
        parent.unwrap_or(0)
    }

    pub fn node(&self, id: usize) -> &HtmlNode {
        &self.nodes[id]
    }
}

pub fn parse(text: &str, void_elements: &[String]) -> HtmlDocument {
    let mut scanner = create_scanner(text, 0, ScannerState::WithinContent, true);

    let mut nodes: Vec<HtmlNode> = Vec::new();

    // Root node (index 0)
    nodes.push(HtmlNode::new(0, text.len(), None));
    let mut curr: usize = 0;

    let mut end_tag_start: Option<usize> = None;
    let mut end_tag_name: Option<String> = None;
    let mut pending_attribute: Option<String> = None;

    let mut token = scanner.scan();
    while token != TokenType::EOS {
        match token {
            TokenType::StartTagOpen => {
                let child_id = nodes.len();
                let child = HtmlNode::new(scanner.get_token_offset(), text.len(), Some(curr));
                nodes.push(child);
                nodes[curr].children.push(child_id);
                curr = child_id;
            }
            TokenType::StartTag => {
                nodes[curr].tag = Some(scanner.get_token_text());
            }
            TokenType::StartTagClose => {
                if let Some(parent_id) = nodes[curr].parent {
                    nodes[curr].end = scanner.get_token_end();
                    if scanner.get_token_length() > 0 {
                        nodes[curr].start_tag_end = Some(scanner.get_token_end());
                        if let Some(ref tag) = nodes[curr].tag.clone() {
                            let is_void = void_elements.iter().any(|v| v.eq_ignore_ascii_case(tag));
                            if is_void {
                                nodes[curr].closed = true;
                                curr = parent_id;
                            }
                        }
                    } else {
                        curr = parent_id;
                    }
                }
            }
            TokenType::StartTagSelfClose => {
                if let Some(parent_id) = nodes[curr].parent {
                    nodes[curr].closed = true;
                    nodes[curr].end = scanner.get_token_end();
                    nodes[curr].start_tag_end = Some(scanner.get_token_end());
                    curr = parent_id;
                }
            }
            TokenType::EndTagOpen => {
                end_tag_start = Some(scanner.get_token_offset());
                end_tag_name = None;
            }
            TokenType::EndTag => {
                end_tag_name = Some(scanner.get_token_text().to_lowercase());
            }
            TokenType::EndTagClose => {
                if let (Some(ets), Some(ref etn)) = (end_tag_start, &end_tag_name) {
                    let etn = etn.clone();
                    // Find matching tag
                    let mut node = curr;
                    loop {
                        if nodes[node].is_same_tag(Some(&etn)) {
                            break;
                        }
                        if let Some(p) = nodes[node].parent {
                            node = p;
                        } else {
                            break;
                        }
                    }
                    if nodes[node].parent.is_some() {
                        while curr != node {
                            nodes[curr].end = ets;
                            nodes[curr].closed = false;
                            curr = nodes[curr].parent.unwrap();
                        }
                        nodes[curr].closed = true;
                        nodes[curr].end_tag_start = Some(ets);
                        nodes[curr].end = scanner.get_token_end();
                        curr = nodes[curr].parent.unwrap();
                    }
                }
            }
            TokenType::AttributeName => {
                let attr_name = scanner.get_token_text();
                nodes[curr].attributes.insert(attr_name.clone(), None);
                pending_attribute = Some(attr_name);
            }
            TokenType::AttributeValue => {
                let value = scanner.get_token_text();
                if let Some(ref attr) = pending_attribute {
                    nodes[curr].attributes.insert(attr.clone(), Some(value));
                    pending_attribute = None;
                }
            }
            _ => {}
        }
        token = scanner.scan();
    }

    // Close unclosed nodes
    while let Some(parent_id) = nodes[curr].parent {
        nodes[curr].end = text.len();
        nodes[curr].closed = false;
        curr = parent_id;
    }

    let roots = nodes[0].children.clone();

    HtmlDocument { nodes, roots }
}
