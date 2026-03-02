use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct HtmlDataV1 {
    pub version: f64,
    #[serde(default)]
    pub tags: Vec<TagData>,
    #[serde(default, rename = "globalAttributes")]
    pub global_attributes: Vec<AttributeData>,
    #[serde(default, rename = "valueSets")]
    pub value_sets: Vec<ValueSet>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TagData {
    pub name: String,
    pub description: Option<MarkupDescription>,
    #[serde(default)]
    pub attributes: Vec<AttributeData>,
    #[serde(default)]
    pub references: Vec<Reference>,
    #[serde(default, rename = "void")]
    pub is_void: Option<bool>,
    #[serde(default)]
    pub browsers: Vec<String>,
    pub status: Option<BaselineStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttributeData {
    pub name: String,
    pub description: Option<MarkupDescription>,
    #[serde(rename = "valueSet")]
    pub value_set: Option<String>,
    #[serde(default)]
    pub values: Vec<ValueData>,
    #[serde(default)]
    pub references: Vec<Reference>,
    #[serde(default)]
    pub browsers: Vec<String>,
    pub status: Option<BaselineStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValueData {
    pub name: String,
    pub description: Option<MarkupDescription>,
    #[serde(default)]
    pub references: Vec<Reference>,
    #[serde(default)]
    pub browsers: Vec<String>,
    pub status: Option<BaselineStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValueSet {
    pub name: String,
    #[serde(default)]
    pub values: Vec<ValueData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MarkupDescription {
    Plain(String),
    Markup { kind: String, value: String },
}

impl MarkupDescription {
    pub fn value(&self) -> &str {
        match self {
            MarkupDescription::Plain(s) => s,
            MarkupDescription::Markup { value, .. } => value,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BaselineStatus {
    pub baseline: serde_json::Value,
    pub baseline_low_date: Option<String>,
    pub baseline_high_date: Option<String>,
}

pub struct HtmlDataProvider {
    id: String,
    tag_map: HashMap<String, TagData>,
    global_attributes: Vec<AttributeData>,
    value_set_map: HashMap<String, Vec<ValueData>>,
}

impl HtmlDataProvider {
    pub fn new(id: &str, data: HtmlDataV1) -> Self {
        let mut tag_map = HashMap::new();
        for tag in &data.tags {
            tag_map.insert(tag.name.to_lowercase(), tag.clone());
        }

        let mut value_set_map = HashMap::new();
        for vs in &data.value_sets {
            value_set_map.insert(vs.name.clone(), vs.values.clone());
        }

        Self {
            id: id.to_string(),
            tag_map,
            global_attributes: data.global_attributes,
            value_set_map,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn provide_tags(&self) -> Vec<&TagData> {
        self.tag_map.values().collect()
    }

    pub fn provide_attributes(&self, tag: &str) -> Vec<&AttributeData> {
        let mut result: Vec<&AttributeData> = Vec::new();

        if let Some(tag_data) = self.tag_map.get(&tag.to_lowercase()) {
            for attr in &tag_data.attributes {
                result.push(attr);
            }
        }

        for attr in &self.global_attributes {
            result.push(attr);
        }

        result
    }

    pub fn provide_values(&self, tag: &str, attribute: &str) -> Vec<&ValueData> {
        let mut result = Vec::new();
        let attr_lower = attribute.to_lowercase();

        // Check tag-specific attributes
        if let Some(tag_data) = self.tag_map.get(&tag.to_lowercase()) {
            for attr in &tag_data.attributes {
                if attr.name.to_lowercase() == attr_lower {
                    if let Some(ref vs_name) = attr.value_set {
                        if let Some(values) = self.value_set_map.get(vs_name) {
                            for v in values {
                                result.push(v);
                            }
                        }
                    }
                    for v in &attr.values {
                        result.push(v);
                    }
                }
            }
        }

        // Check global attributes
        for attr in &self.global_attributes {
            if attr.name.to_lowercase() == attr_lower {
                if let Some(ref vs_name) = attr.value_set {
                    if let Some(values) = self.value_set_map.get(vs_name) {
                        for v in values {
                            result.push(v);
                        }
                    }
                }
                for v in &attr.values {
                    result.push(v);
                }
            }
        }

        result
    }
}
