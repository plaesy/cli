//! Native plugin install plans, one per AI platform.
//!
//! A plan is data: the commands the platform's own CLI runs, and the steps a person must
//! do by hand when the platform has no non-interactive install. Nothing here runs a
//! command; the caller decides whether and how to execute a plan.

/// What it takes to install the plugin into one platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    /// Commands run in order, each as an argument vector (program first).
    pub commands: Vec<Vec<String>>,
    /// Steps a person completes in the platform's UI, after the commands.
    pub manual_steps: Vec<String>,
}

/// Plugin name and marketplace name that the spec-kit repository publishes.
pub const PLUGIN: &str = "spec-kit@spec-kit";

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| (*p).to_owned()).collect()
}

/// The install plan for `platform`, given where the spec-kit plugin lives (a local path or
/// a git URL). Returns `None` for a platform with no native route.
#[must_use]
pub fn plan(platform: &str, source: &str) -> Option<InstallPlan> {
    match platform {
        "claude" => Some(InstallPlan {
            commands: vec![
                argv(&["claude", "plugin", "marketplace", "add", source]),
                argv(&["claude", "plugin", "install", PLUGIN]),
            ],
            manual_steps: Vec::new(),
        }),
        "github_copilot" => Some(InstallPlan {
            commands: vec![
                argv(&["copilot", "plugin", "marketplace", "add", source]),
                argv(&["copilot", "plugin", "install", PLUGIN]),
            ],
            manual_steps: Vec::new(),
        }),
        "gemini" => Some(InstallPlan {
            commands: vec![argv(&["gemini", "extensions", "install", source])],
            manual_steps: Vec::new(),
        }),
        // Cursor can register a marketplace from the shell, but the plugin itself is
        // installed in the editor, so the last step stays with the person.
        "cursor_ai" => Some(InstallPlan {
            commands: vec![argv(&["cursor-agent", "plugin", "marketplace", "add", source])],
            manual_steps: vec!["In Cursor Agent chat, run: /add-plugin spec-kit".to_owned()],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_registers_the_marketplace_then_installs_the_plugin() {
        let p = plan("claude", "/src/spec-kit").unwrap();
        assert_eq!(p.commands[0], argv(&["claude", "plugin", "marketplace", "add", "/src/spec-kit"]));
        assert_eq!(p.commands[1], argv(&["claude", "plugin", "install", PLUGIN]));
        assert!(p.manual_steps.is_empty());
    }

    #[test]
    fn cursor_needs_a_manual_last_step() {
        let p = plan("cursor_ai", "/src/spec-kit").unwrap();
        assert_eq!(p.commands.len(), 1);
        assert_eq!(p.manual_steps.len(), 1);
    }

    #[test]
    fn platforms_without_a_native_route_have_no_plan() {
        assert!(plan("aider", "/src/spec-kit").is_none());
        assert!(plan("kilo", "/src/spec-kit").is_none());
    }
}
