use once_cell::sync::Lazy;

use super::data_provider::{HtmlDataProvider, HtmlDataV1};

static HTML_DATA: Lazy<HtmlDataV1> = Lazy::new(|| {
    let json_str = include_str!("web_custom_data.json");
    serde_json::from_str(json_str).expect("Failed to parse web_custom_data.json")
});

static VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "keygen",
    "link", "menuitem", "meta", "param", "source", "track", "wbr",
];

/// Maps tag -> list of attributes that hold file paths
static PATH_ATTRIBUTES: Lazy<Vec<(&str, &str)>> = Lazy::new(|| {
    vec![
        ("*", "href"),
        ("*", "src"),
        ("*", "srcset"),
        ("a", "ping"),
        ("audio", "src"),
        ("blockquote", "cite"),
        ("body", "background"),
        ("button", "formaction"),
        ("del", "cite"),
        ("form", "action"),
        ("html", "manifest"),
        ("iframe", "src"),
        ("img", "src"),
        ("img", "srcset"),
        ("input", "formaction"),
        ("input", "src"),
        ("ins", "cite"),
        ("link", "href"),
        ("object", "data"),
        ("q", "cite"),
        ("script", "src"),
        ("source", "src"),
        ("source", "srcset"),
        ("track", "src"),
        ("video", "poster"),
        ("video", "src"),
    ]
});

pub struct DataManager {
    provider: HtmlDataProvider,
}

impl DataManager {
    pub fn new() -> Self {
        let provider = HtmlDataProvider::new("default", HTML_DATA.clone());
        Self { provider }
    }

    pub fn provider(&self) -> &HtmlDataProvider {
        &self.provider
    }

    pub fn is_void_element(&self, tag: &str) -> bool {
        let lower = tag.to_lowercase();
        VOID_ELEMENTS.iter().any(|v| *v == lower)
    }

    pub fn get_void_elements(&self) -> Vec<String> {
        VOID_ELEMENTS.iter().map(|s| s.to_string()).collect()
    }

    pub fn is_path_attribute(&self, tag: &str, attribute: &str) -> bool {
        let tag_lower = tag.to_lowercase();
        let attr_lower = attribute.to_lowercase();
        PATH_ATTRIBUTES.iter().any(|(t, a)| {
            (*t == "*" || *t == tag_lower) && *a == attr_lower
        })
    }
}
