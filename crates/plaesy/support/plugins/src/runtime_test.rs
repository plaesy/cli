use super::*;
use crate::error::PluginError;
use crate::limits::DEFAULT_MEMORY_BYTES;

const MANIFEST: &str = r#"{"name":"notes-sync","version":"0.1.0","abi":"1.0","capabilities":[{"kind":"fs_read","path":"/data/notes"},{"kind":"net_host","host":"api.example.com"},{"kind":"clock"}]}"#;

#[test]
fn plan_grants_only_the_intersection() {
    let approved = [
        Capability::FsRead {
            path: "/data".to_owned(),
        },
        Capability::Clock,
    ];
    let plan = plan_load(MANIFEST, &approved, &HostConfig::default()).unwrap();
    assert_eq!(plan.manifest.name.as_str(), "notes-sync");
    assert_eq!(
        plan.grant.granted,
        vec![
            Capability::FsRead {
                path: "/data/notes".to_owned()
            },
            Capability::Clock,
        ]
    );
    assert_eq!(
        plan.grant.denied,
        vec![Capability::NetHost {
            host: "api.example.com".to_owned()
        }]
    );
    assert_eq!(plan.limits, ResourceLimits::default());
}

#[test]
fn approval_paths_are_normalized_before_matching() {
    let approved = [Capability::FsRead {
        path: r"\data\".to_owned(),
    }];
    let plan = plan_load(MANIFEST, &approved, &HostConfig::default()).unwrap();
    assert_eq!(
        plan.grant.granted,
        vec![Capability::FsRead {
            path: "/data/notes".to_owned()
        }]
    );
}

#[test]
fn invalid_approval_is_rejected() {
    let approved = [Capability::FsRead {
        path: "/data/../etc".to_owned(),
    }];
    assert!(matches!(
        plan_load(MANIFEST, &approved, &HostConfig::default()),
        Err(PluginError::InvalidCapability(_))
    ));
}

#[test]
fn abi_mismatch_is_rejected() {
    let manifest = MANIFEST.replace(r#""abi":"1.0""#, r#""abi":"2.0""#);
    assert!(matches!(
        plan_load(&manifest, &[], &HostConfig::default()),
        Err(PluginError::UnsupportedAbi { .. })
    ));
}

#[test]
fn invalid_host_limits_fail_before_manifest_is_read() {
    let host = HostConfig {
        abi: HOST_ABI,
        limits: ResourceLimits {
            fuel: 0,
            memory_bytes: DEFAULT_MEMORY_BYTES,
        },
    };
    assert!(matches!(
        plan_load("not json", &[], &host),
        Err(PluginError::InvalidLimits(_))
    ));
}

struct FakeRuntime;

impl PluginRuntime for FakeRuntime {
    type Instance = (String, usize);

    fn instantiate(&self, plan: &LoadPlan, module: &[u8]) -> Result<Self::Instance, PluginError> {
        if module.is_empty() {
            return Err(PluginError::Runtime("modul kosong".to_owned()));
        }
        Ok((plan.manifest.name.to_string(), plan.grant.granted.len()))
    }
}

#[test]
fn runtime_seam_receives_the_plan() {
    let plan = plan_load(MANIFEST, &[Capability::Clock], &HostConfig::default()).unwrap();
    let instance = FakeRuntime.instantiate(&plan, b"\0asm").unwrap();
    assert_eq!(instance, ("notes-sync".to_owned(), 1));
    assert!(matches!(
        FakeRuntime.instantiate(&plan, &[]),
        Err(PluginError::Runtime(_))
    ));
}
