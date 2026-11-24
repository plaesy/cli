use super::*;
use crate::abi::Abi;
use crate::capability::Capability;
use crate::error::PluginError;

const VALID: &str = r#"{"name":"notes-sync","version":"0.1.0","abi":"1.0","capabilities":[{"kind":"fs_read","path":"C:\\data\\notes"},{"kind":"net_host","host":"API.Example.com"},{"kind":"clock"}]}"#;

fn manifest_json(name: &str, version: &str, abi: &str, capabilities: &str) -> String {
    format!(
        r#"{{"name":"{name}","version":"{version}","abi":"{abi}","capabilities":{capabilities}}}"#
    )
}

#[test]
fn parses_valid_manifest_and_normalizes_capabilities() {
    let manifest = Manifest::parse(VALID).unwrap();
    assert_eq!(manifest.name.as_str(), "notes-sync");
    assert_eq!(
        manifest.version,
        Version {
            major: 0,
            minor: 1,
            patch: 0
        }
    );
    assert_eq!(manifest.abi, Abi::new(1, 0));
    assert_eq!(
        manifest.capabilities,
        vec![
            Capability::FsRead {
                path: "C:/data/notes".to_owned()
            },
            Capability::NetHost {
                host: "api.example.com".to_owned()
            },
            Capability::Clock,
        ]
    );
}

#[test]
fn capabilities_default_to_empty() {
    let manifest = Manifest::parse(r#"{"name":"a","version":"1.2.3","abi":"1.0"}"#).unwrap();
    assert!(manifest.capabilities.is_empty());
}

#[test]
fn rejects_malformed_json_and_unknown_fields() {
    assert!(matches!(Manifest::parse("{not json"), Err(PluginError::Manifest(_))));
    let extra = r#"{"name":"a","version":"1.0.0","abi":"1.0","permisi":[]}"#;
    assert!(matches!(Manifest::parse(extra), Err(PluginError::Manifest(_))));
    let unknown_kind = manifest_json("a", "1.0.0", "1.0", r#"[{"kind":"exec"}]"#);
    assert!(matches!(
        Manifest::parse(&unknown_kind),
        Err(PluginError::Manifest(_))
    ));
}

#[test]
fn validates_plugin_name() {
    for name in ["Notes", "9a", "-a", "a-", "a_b", "", "a b"] {
        let json = manifest_json(name, "1.0.0", "1.0", "[]");
        assert_eq!(
            Manifest::parse(&json),
            Err(PluginError::InvalidName(name.to_owned())),
            "{name:?}"
        );
    }
    let longest = "a".repeat(MAX_NAME_LEN);
    assert!(Manifest::parse(&manifest_json(&longest, "1.0.0", "1.0", "[]")).is_ok());
    let too_long = "a".repeat(MAX_NAME_LEN + 1);
    assert!(matches!(
        Manifest::parse(&manifest_json(&too_long, "1.0.0", "1.0", "[]")),
        Err(PluginError::InvalidName(_))
    ));
}

#[test]
fn validates_version_and_abi() {
    for version in ["1.0", "1.x.0", "1.0.0.0", "v1.0.0"] {
        let json = manifest_json("a", version, "1.0", "[]");
        assert_eq!(
            Manifest::parse(&json),
            Err(PluginError::InvalidVersion(version.to_owned())),
            "{version:?}"
        );
    }
    let json = manifest_json("a", "1.0.0", "one", "[]");
    assert_eq!(
        Manifest::parse(&json),
        Err(PluginError::InvalidAbi("one".to_owned()))
    );
}

#[test]
fn rejects_duplicate_capability_after_normalization() {
    let clocks = manifest_json("a", "1.0.0", "1.0", r#"[{"kind":"clock"},{"kind":"clock"}]"#);
    assert_eq!(
        Manifest::parse(&clocks),
        Err(PluginError::DuplicateCapability("clock".to_owned()))
    );
    let paths = manifest_json(
        "a",
        "1.0.0",
        "1.0",
        r#"[{"kind":"fs_read","path":"/a/"},{"kind":"fs_read","path":"/a"}]"#,
    );
    assert_eq!(
        Manifest::parse(&paths),
        Err(PluginError::DuplicateCapability("fs_read:/a".to_owned()))
    );
}

#[test]
fn rejects_unknown_field_inside_capability() {
    let json = manifest_json(
        "a",
        "1.0.0",
        "1.0",
        r#"[{"kind":"fs_read","path":"/a","mode":"rw"}]"#,
    );
    assert!(matches!(
        Manifest::parse(&json),
        Err(PluginError::Manifest(_))
    ));
}

#[test]
fn rejects_too_many_capabilities() {
    let items: Vec<String> = (0..=MAX_CAPABILITIES)
        .map(|i| format!(r#"{{"kind":"fs_read","path":"/p{i}"}}"#))
        .collect();
    let json = manifest_json("a", "1.0.0", "1.0", &format!("[{}]", items.join(",")));
    assert!(matches!(
        Manifest::parse(&json),
        Err(PluginError::InvalidCapability(_))
    ));
}

#[test]
fn rejects_unsafe_capability_values() {
    let traversal = manifest_json(
        "a",
        "1.0.0",
        "1.0",
        r#"[{"kind":"fs_read","path":"/a/../b"}]"#,
    );
    assert!(matches!(
        Manifest::parse(&traversal),
        Err(PluginError::InvalidCapability(_))
    ));
    let wildcard = manifest_json(
        "a",
        "1.0.0",
        "1.0",
        r#"[{"kind":"net_host","host":"*.example.com"}]"#,
    );
    assert!(matches!(
        Manifest::parse(&wildcard),
        Err(PluginError::InvalidCapability(_))
    ));
}
