use super::render_unit;
use std::path::Path;

#[test]
fn unit_runs_serve_and_restarts_always() {
    let unit = render_unit(
        Path::new("/usr/local/bin/plaesy"),
        Path::new("/home/ubuntu/_"),
        9123,
    );
    assert!(unit.contains(
        "ExecStart=\"/usr/local/bin/plaesy\" mcp memory serve --vault \"/home/ubuntu/_\" --port 9123"
    ));
    assert!(unit.contains("Restart=on-failure"));
    assert!(unit.contains("RestartSec=30"));
    assert!(unit.contains("WantedBy=default.target"));
}

#[test]
fn unit_never_embeds_secrets_and_loads_env_file() {
    let unit = render_unit(
        Path::new("/usr/local/bin/plaesy"),
        Path::new("/home/ubuntu/_"),
        9123,
    );
    assert!(unit.contains("EnvironmentFile=-%h/.config/plaesy/memory-mcp/env"));
    assert!(!unit.contains("PLAESY_MCP_TOKEN="));
    assert!(!unit.contains("--auth-token"));
}

#[test]
fn unit_escapes_percent_and_quotes_in_paths() {
    let unit = render_unit(
        Path::new("/opt/a b/plaesy"),
        Path::new("/home/u/vault%x\"y"),
        9200,
    );
    assert!(unit.contains("\"/opt/a b/plaesy\""));
    assert!(unit.contains("vault%%x\\\"y"));
}
