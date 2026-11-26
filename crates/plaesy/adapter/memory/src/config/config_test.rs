//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use std::path::Path;

#[tokio::test]
async fn test_load_from_file_merges_over_defaults() {
    let dir = tempfile::tempdir().expect("tmp");
    let p = dir.path().join("config.toml");
    std::fs::write(
        &p,
        "[search]\ndefault_limit = 5\n[embeddings]\nmodel_dir = \"/models/x\"\n",
    )
    .expect("write");
    let cfg = MemoryConfig::load_from_file(&p).await.expect("load");
    assert_eq!(cfg.search.default_limit, 5);
    assert_eq!(cfg.search.max_limit, 100); // untouched default
    assert_eq!(cfg.embeddings.model_dir, Some(PathBuf::from("/models/x")));
}

#[tokio::test]
async fn test_load_from_file_errors() {
    let dir = tempfile::tempdir().expect("tmp");
    let missing = dir.path().join("nope.toml");
    assert!(MemoryConfig::load_from_file(&missing).await.is_err());
    let bad = dir.path().join("bad.toml");
    std::fs::write(&bad, "[search").expect("write");
    assert!(MemoryConfig::load_from_file(&bad).await.is_err());
    let wrong_type = dir.path().join("t.toml");
    std::fs::write(&wrong_type, "[search]\ndefault_limit = \"ten\"\n").expect("write");
    assert!(MemoryConfig::load_from_file(&wrong_type).await.is_err());
    let invalid = dir.path().join("v.toml");
    std::fs::write(&invalid, "[search]\ndefault_limit = 500\n").expect("write");
    assert!(MemoryConfig::load_from_file(&invalid).await.is_err());
}

#[tokio::test]
async fn test_load_from_file_reads_toml_by_extension() {
    let dir = tempfile::tempdir().expect("tmp");
    let p = dir.path().join("config.toml");
    std::fs::write(&p, "[search]\ndefault_limit = 7\n").expect("write");
    let cfg = MemoryConfig::load_from_file(&p).await.expect("load");
    assert_eq!(cfg.search.default_limit, 7);
    assert_eq!(cfg.search.max_limit, 100); // untouched default
    let bad = dir.path().join("bad.toml");
    std::fs::write(&bad, "[search\n").expect("write");
    assert!(MemoryConfig::load_from_file(&bad).await.is_err());
}

#[tokio::test]
async fn test_mcp_settings_load_from_toml() {
    let dir = tempfile::tempdir().expect("tmp");
    let p = dir.path().join("config.toml");
    std::fs::write(
        &p,
        "[mcp]\ntransport = \"http\"\nport = 9123\nauth_token_env = \"MY_MCP_TOKEN\"\nread_only = true\n",
    )
    .expect("write");
    let cfg = MemoryConfig::load_from_file(&p).await.expect("load");
    assert_eq!(cfg.mcp.transport, "http");
    assert_eq!(cfg.mcp.port, 9123);
    assert_eq!(cfg.mcp.auth_token_env, "MY_MCP_TOKEN");
    assert!(cfg.mcp.read_only);
}

#[test]
fn test_apply_env_overrides_and_rejects_garbage() {
    let mut cfg = MemoryConfig::default();
    let env: std::collections::HashMap<&str, &str> = [
        ("MEMORY_SEARCH_MAX_LIMIT", "50"),
        ("MEMORY_LOG_JSON", "true"),
        ("MEMORY_DB_PATH", "x/state.db"),
    ]
    .into_iter()
    .collect();
    cfg.apply_env(|k| env.get(k).map(|s| s.to_string()))
        .expect("env");
    assert_eq!(cfg.search.max_limit, 50);
    assert!(cfg.logging.json_format);
    assert_eq!(cfg.database.db_path, PathBuf::from("x/state.db"));

    let mut cfg = MemoryConfig::default();
    let r = cfg.apply_env(|k| (k == "MEMORY_SEARCH_TIMEOUT_SECS").then(|| "soon".to_string()));
    assert!(r.is_err());
}

#[test]
fn test_default_config() {
    let config = MemoryConfig::default();
    assert_eq!(config.watch_dir, PathBuf::from(".plaesy"));
    assert_eq!(config.search.default_limit, 10);
    assert!(config
        .database
        .db_path
        .ends_with(Path::new("memory").join("storage.db")));
}

#[test]
fn test_validate_config_valid() {
    let config = MemoryConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_validate_config_invalid_limits() {
    let mut config = MemoryConfig::default();
    config.search.default_limit = 200;
    config.search.max_limit = 100;
    assert!(config.validate().is_err());
}

#[test]
fn test_validate_config_rejects_invalid_mcp_settings() {
    let mut config = MemoryConfig::default();
    config.mcp.transport = "websocket".to_owned();
    assert!(config.validate().is_err());

    config.mcp.transport = "http".to_owned();
    config.mcp.port = 0;
    assert!(config.validate().is_err());
}

#[test]
fn test_validate_config_invalid_confidence() {
    let mut config = MemoryConfig::default();
    config.search.confidence_threshold = 1.5;
    assert!(config.validate().is_err());
}

#[test]
fn test_validate_config_invalid_dimension() {
    let mut config = MemoryConfig::default();
    config.embeddings.dimension = 0;
    assert!(config.validate().is_err());
}

#[test]
fn test_diagnostic_string() {
    let config = MemoryConfig::default();
    let diag = config.to_diagnostic_string();
    assert!(diag.contains("Watch Dir"));
    assert!(diag.contains("DB Path"));
    assert!(diag.contains("Memory Config"));
}
