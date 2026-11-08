//! Graph data types, the node-type and architecture-layer classifiers.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::extensions::is_source_ext;

/// One node: a file in the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub group: String,
    pub degree: usize,
    pub community: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layer: String,
    #[serde(default)]
    pub symbols: Vec<String>,
}

/// One edge between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// `EXTRACTED` or `INFERRED`.
    pub confidence: String,
}

/// A full build result, also the shape of `project.graph.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graph {
    pub generated_at: String,
    pub root: String,
    pub description: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// The type a file's path gives it.
#[must_use]
pub fn node_type_of(rel: &str) -> &'static str {
    let top = rel.split('/').next().unwrap_or(rel);
    match top {
        "agents" => "agent",
        "instructions" => "instruction",
        "checklists" => "checklist",
        "templates" => "template",
        "scripts" => "script",
        "docs" => "doc",
        "prompts" => "prompt",
        "testing" => "testing",
        _ if is_source_ext(rel) => "source",
        _ => "other",
    }
}

macro_rules! layer_regex {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect("static regex"))
        }
    };
}

layer_regex!(
    api_folder,
    r"^(api|controllers|handlers|routes|endpoints|gateway)$"
);
layer_regex!(
    api_file,
    r"(controller|handler|route|gateway|middleware|app|server)"
);
layer_regex!(api_path, r"/(api|controllers|routes|handlers)/");
layer_regex!(
    svc_folder,
    r"^(services|business|use-cases|use_cases|orchestration|workflows)$"
);
layer_regex!(
    svc_file,
    r"(service|business|usecase|workflow|orchestration)"
);
layer_regex!(svc_path, r"/(services|business|use[-_]cases)/");
layer_regex!(
    data_folder,
    r"^(repositories|repo|models|database|db|schemas|data|persistence)$"
);
layer_regex!(
    data_file,
    r"(repo|repository|model|entity|schema|mapper|query|dao)"
);
layer_regex!(
    data_path,
    r"/(repositories|models|database|db|persistence|entities)/"
);
layer_regex!(
    ui_folder,
    r"^(components|views|pages|screens|ui|presentation|widgets)$"
);
layer_regex!(ui_file, r"(component|view|page|screen|widget)");
layer_regex!(ui_path, r"/(components|views|pages|screens|ui)/");
layer_regex!(
    util_folder,
    r"^(utils|helpers|common|lib|tools|config|constants)$"
);
layer_regex!(
    util_file,
    r"(util|helper|common|constant|config|logger|validator|formatter)"
);
layer_regex!(util_path, r"/(utils|helpers|common|config|constants)/");

/// The architecture layer a file belongs to, or `""` when none matches.
#[must_use]
pub fn architecture_layer(rel: &str) -> &'static str {
    let lower = rel.to_lowercase();
    let folder = rel.split('/').next().unwrap_or(rel);
    let file_part = rel.rsplit('/').next().unwrap_or(rel);
    let stem = file_part
        .rsplit_once('.')
        .map_or(file_part, |(stem, _)| stem)
        .to_lowercase();
    let is = |re: &Regex, text: &str| re.is_match(text);
    if is(api_folder(), folder) || is(api_file(), &stem) || is(api_path(), &lower) {
        "API"
    } else if is(svc_folder(), folder) || is(svc_file(), &stem) || is(svc_path(), &lower) {
        "Service"
    } else if is(data_folder(), folder) || is(data_file(), &stem) || is(data_path(), &lower) {
        "Data"
    } else if is(ui_folder(), folder) || is(ui_file(), &stem) || is(ui_path(), &lower) {
        "UI"
    } else if is(util_folder(), folder) || is(util_file(), &stem) || is(util_path(), &lower) {
        "Utility"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_type_follows_the_top_folder_then_the_extension() {
        assert_eq!(node_type_of("agents/x.agents.md"), "agent");
        assert_eq!(node_type_of("src/main.rs"), "source");
        assert_eq!(node_type_of("README.txt"), "other");
    }

    #[test]
    fn architecture_layer_reads_folders_then_names() {
        assert_eq!(architecture_layer("api/users.go"), "API");
        assert_eq!(architecture_layer("models/user.rs"), "Data");
        assert_eq!(architecture_layer("docs/readme.md"), "");
    }
}
