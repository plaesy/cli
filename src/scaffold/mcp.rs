//! Registers the read-only `plaesy` MCP server in the project's `.mcp.json`.

use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::common::log::Logger;

use super::{Policy, ScaffoldError};

/// File Claude Code reads from the project root.
pub const MCP_CONFIG_FILE: &str = ".mcp.json";

/// Key the server is registered under.
pub const MCP_SERVER_NAME: &str = "plaesy";

/// Adds or refreshes the `plaesy` entry in `<target>/.mcp.json`. Other entries are
/// kept. A file that is not valid JSON is left alone and reported, so a user's
/// file is never overwritten.
///
/// # Errors
///
/// I/O errors reading or writing the file.
pub fn install_mcp_server(
    target_dir: &Path,
    policy: &Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let path = target_dir.join(MCP_CONFIG_FILE);
    let mut root: Map<String, Value> = match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map,
            _ => {
                log.warning(&format!(
                    "  ⊘ {} is not valid JSON; MCP server not registered, fix the file and run reload",
                    path.display()
                ));
                return Ok(());
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(ScaffoldError::io(format!("reading {}", path.display()), e)),
    };

    if policy.dry_run {
        return Ok(());
    }

    let mut servers = match root.remove("mcpServers") {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    servers.insert(
        MCP_SERVER_NAME.to_owned(),
        json!({ "command": "plaesy", "args": ["mcp"] }),
    );
    root.insert("mcpServers".to_owned(), Value::Object(servers));

    let mut text = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| ScaffoldError::Config(format!("encoding {}: {e}", path.display())))?;
    text.push('\n');
    fs::write(&path, text)
        .map_err(|e| ScaffoldError::io(format!("writing {}", path.display()), e))?;
    log.success(&format!("  ✓ plaesy MCP server → {MCP_CONFIG_FILE}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_the_server_and_keeps_other_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MCP_CONFIG_FILE);
        fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        install_mcp_server(dir.path(), &Policy::default(), &Logger::new("t")).unwrap();
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["mcpServers"]["plaesy"]["args"][0], "mcp");
        assert_eq!(value["mcpServers"]["other"]["command"], "x");
    }

    #[test]
    fn invalid_json_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MCP_CONFIG_FILE);
        fs::write(&path, "{ not json").unwrap();
        install_mcp_server(dir.path(), &Policy::default(), &Logger::new("t")).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
