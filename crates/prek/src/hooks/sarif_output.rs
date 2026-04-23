use std::path::Path;

use anyhow::Result;
use serde_json::json;

#[derive(Debug, Clone)]
pub(crate) struct HookDiagnostic {
    pub(crate) rule_id: &'static str,
    pub(crate) message: String,
    pub(crate) path: String,
    pub(crate) start_line: Option<u64>,
    pub(crate) start_column: Option<u64>,
}

impl HookDiagnostic {
    pub(crate) fn new(rule_id: &'static str, path: &Path, message: String) -> Self {
        Self {
            rule_id,
            message,
            path: path.display().to_string(),
            start_line: None,
            start_column: None,
        }
    }

    pub(crate) fn with_location(mut self, line: Option<u64>, column: Option<u64>) -> Self {
        self.start_line = line;
        self.start_column = column;
        self
    }
}

pub(crate) fn render_sarif(
    tool_name: &str,
    hook_id: &str,
    diagnostics: &[HookDiagnostic],
) -> Result<Vec<u8>> {
    let mut rules = Vec::new();
    let mut seen_rule_ids = std::collections::BTreeSet::new();
    let mut results = Vec::new();

    for diagnostic in diagnostics {
        if seen_rule_ids.insert(diagnostic.rule_id) {
            rules.push(json!({
                "id": diagnostic.rule_id,
                "name": diagnostic.rule_id,
                "shortDescription": {
                    "text": format!("{hook_id} violation"),
                },
            }));
        }

        let mut region = serde_json::Map::new();
        if let Some(line) = diagnostic.start_line {
            region.insert("startLine".to_string(), json!(line));
        }
        if let Some(column) = diagnostic.start_column {
            region.insert("startColumn".to_string(), json!(column));
        }

        let mut location = json!({
            "physicalLocation": {
                "artifactLocation": {
                    "uri": diagnostic.path,
                },
            },
        });
        if !region.is_empty() {
            location["physicalLocation"]["region"] = json!(region);
        }

        results.push(json!({
            "ruleId": diagnostic.rule_id,
            "level": "error",
            "message": {
                "text": diagnostic.message,
            },
            "locations": [location],
        }));
    }

    let run = json!({
        "tool": {
            "driver": {
                "name": tool_name,
                "rules": rules,
            },
        },
        "results": results,
    });
    Ok(serde_json::to_vec(&run)?)
}
