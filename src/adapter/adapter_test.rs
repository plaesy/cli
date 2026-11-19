use super::*;

#[test]
fn adapter_conformance_success_requires_evidence() {
    assert_eq!(
        outcome_violations(&Outcome::Succeeded {
            evidence: "  ".into()
        }),
        vec!["succeeded without evidence".to_owned()]
    );
    assert!(outcome_violations(&Outcome::Succeeded {
        evidence: "sha256:abc".into()
    })
    .is_empty());
}

#[test]
fn adapter_conformance_failure_requires_reason() {
    assert_eq!(
        outcome_violations(&Outcome::Failed {
            reason: String::new()
        }),
        vec!["failed without a reason".to_owned()]
    );
}

#[test]
fn adapter_conformance_fake_adapter_conforms() {
    let mut fake = FakeAdapter::new(Script::Succeed("evidence-1".into()));
    assert!(check_conformance(&mut fake).is_empty());
}
