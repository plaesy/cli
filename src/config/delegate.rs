//! Delegate provider settings: which providers `plaesy delegate` may call.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which providers `plaesy delegate` may call.
///
/// Only providers with `enabled` run. A `billable` provider also needs
/// `--confirm` on every run. `chains` names ordered fallbacks, tried only on
/// classified transient or rate-limit errors.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DelegateConfig {
    pub providers: Vec<DelegateProviderConfig>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub chains: BTreeMap<String, Vec<String>>,
}

/// One provider entry.
///
/// `kind` selects the implementation: `cli` (runs `command` with the prompt on
/// stdin), `anthropic` (Messages API), `ollama` (local HTTP), or `openai`
/// (OpenAI-compatible chat completions). Fields a kind does not use are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DelegateProviderConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub enabled: bool,
    pub billable: bool,
    /// cli: executable name or path.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    /// cli: arguments placed before the prompt.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// anthropic, ollama, openai: model id.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// ollama, openai: base URL.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
    /// anthropic, openai: name of the environment variable that holds the key.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_key_env: String,
    /// Daily input plus output token cap for a billable provider. `0` means no cap.
    #[serde(skip_serializing_if = "is_zero")]
    pub daily_token_budget: i64,
}

impl DelegateConfig {
    /// Built-in providers. CLIs run on the user's own subscription and are not
    /// billable. The Anthropic API is billable. Only `claude-cli` starts enabled.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            providers: vec![
                DelegateProviderConfig {
                    name: "claude-cli".into(),
                    kind: "cli".into(),
                    enabled: true,
                    command: "claude".into(),
                    args: vec!["-p".into()],
                    ..Default::default()
                },
                DelegateProviderConfig {
                    name: "kilo-cli".into(),
                    kind: "cli".into(),
                    command: "kilo".into(),
                    args: vec!["run".into()],
                    ..Default::default()
                },
                DelegateProviderConfig {
                    name: "anthropic".into(),
                    kind: "anthropic".into(),
                    billable: true,
                    model: "claude-sonnet-5-5".into(),
                    api_key_env: "ANTHROPIC_API_KEY".into(),
                    ..Default::default()
                },
                DelegateProviderConfig {
                    name: "ollama".into(),
                    kind: "ollama".into(),
                    model: "llama3.2".into(),
                    endpoint: "http://localhost:11434".into(),
                    ..Default::default()
                },
            ],
            chains: BTreeMap::new(),
        }
    }
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_claude_cli_starts_enabled_and_no_provider_is_billable_except_anthropic() {
        let cfg = DelegateConfig::defaults();
        let enabled: Vec<_> = cfg
            .providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(enabled, ["claude-cli"]);
        let billable: Vec<_> = cfg
            .providers
            .iter()
            .filter(|p| p.billable)
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(billable, ["anthropic"]);
    }

    #[test]
    fn type_field_round_trips() {
        let p = DelegateProviderConfig {
            kind: "cli".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""type":"cli""#), "{json}");
        let back: DelegateProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
}
