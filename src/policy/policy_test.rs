use super::*;

#[test]
fn approval_secret_internal_action_always_passes() {
    assert_eq!(may_run(Effect::Internal, false, false), Ok(()));
}

#[test]
fn approval_secret_external_blocked_when_flag_off() {
    assert_eq!(
        may_run(Effect::External, true, false),
        Err(Block::ExternalEffectsDisabled)
    );
}

#[test]
fn approval_secret_external_requires_explicit_approval() {
    assert_eq!(
        may_run(Effect::External, false, true),
        Err(Block::ApprovalRequired)
    );
    assert_eq!(may_run(Effect::External, true, true), Ok(()));
}
