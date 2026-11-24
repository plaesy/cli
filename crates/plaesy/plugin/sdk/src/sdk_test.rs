use super::*;
use serde_json::json;

/// Plugin uji: meminta kapabilitas tertentu, lalu mengembalikan input dengan tanda.
struct Echo {
    name: &'static str,
    needs: Vec<Capability>,
}

impl Plugin for Echo {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: self.name.to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: self.needs.iter().copied().collect(),
        }
    }

    fn run(&self, ctx: &Context<'_>, input: &Value) -> Result<Value, String> {
        for capability in &self.needs {
            ctx.require(*capability).map_err(|e| e.to_string())?;
        }
        Ok(json!({ "echo": input }))
    }
}

/// Plugin yang mencoba memakai kapabilitas yang tidak dideklarasikan di manifest.
struct Sneaky;

impl Plugin for Sneaky {
    fn manifest(&self) -> Manifest {
        Manifest {
            name: "sneaky".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: BTreeSet::new(),
        }
    }

    fn run(&self, ctx: &Context<'_>, _input: &Value) -> Result<Value, String> {
        ctx.require(Capability::Process)
            .map_err(|e| e.to_string())?;
        Ok(json!("tidak seharusnya sampai sini"))
    }
}

fn echo(name: &'static str, needs: &[Capability]) -> Box<dyn Plugin> {
    Box::new(Echo {
        name,
        needs: needs.to_vec(),
    })
}

#[test]
fn sdk_plugin_with_granted_capabilities_loads_and_runs() {
    let mut grants = Grants::default();
    grants.grant("reader", &[Capability::ReadDocuments]);
    let mut host = Host::new(grants);
    host.load(echo("reader", &[Capability::ReadDocuments]))
        .unwrap();
    assert_eq!(host.loaded(), ["reader"]);
    assert_eq!(host.run("reader", &json!(7)).unwrap(), json!({ "echo": 7 }));
}

#[test]
fn sdk_load_refuses_a_capability_that_was_not_granted() {
    let mut host = Host::new(Grants::default());
    let error = host.load(echo("net", &[Capability::Network])).unwrap_err();
    assert_eq!(
        error,
        PluginError::CapabilityDenied {
            plugin: "net".to_owned(),
            capability: Capability::Network,
        }
    );
    assert!(host.loaded().is_empty());
}

#[test]
fn sdk_grant_is_per_plugin_not_global() {
    let mut grants = Grants::default();
    grants.grant("a", &[Capability::Network]);
    let mut host = Host::new(grants);
    assert!(host.load(echo("b", &[Capability::Network])).is_err());
    assert!(host.load(echo("a", &[Capability::Network])).is_ok());
}

#[test]
fn sdk_run_refuses_undeclared_capability_at_runtime() {
    let mut grants = Grants::default();
    grants.grant("sneaky", &[Capability::Process]);
    let mut host = Host::new(grants);
    host.load(Box::new(Sneaky)).unwrap();
    let error = host.run("sneaky", &json!(null)).unwrap_err();
    assert!(matches!(error, PluginError::Failed { ref plugin, .. } if plugin == "sneaky"));
    assert!(error.to_string().contains("process"));
}

#[test]
fn sdk_duplicate_name_is_refused() {
    let mut grants = Grants::default();
    grants.grant("dup", &[]);
    let mut host = Host::new(grants);
    host.load(echo("dup", &[])).unwrap();
    assert_eq!(
        host.load(echo("dup", &[])).unwrap_err(),
        PluginError::Duplicate("dup".to_owned())
    );
}

#[test]
fn sdk_unknown_plugin_is_reported() {
    let host = Host::new(Grants::default());
    assert_eq!(
        host.run("ada-tidak", &json!(null)).unwrap_err(),
        PluginError::Unknown("ada-tidak".to_owned())
    );
}
