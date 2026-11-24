//! Unit tests for `update.rs`: version parsing, ordering, comparison, and manifest reading.

use super::*;

fn v(s: &str) -> Version {
    s.parse().unwrap()
}

#[test]
fn parse_accepts_strict_semver_with_optional_v_and_whitespace() {
    assert_eq!(
        v("1.2.3"),
        Version {
            major: 1,
            minor: 2,
            patch: 3
        }
    );
    assert_eq!(
        v(" 1.2.3\n"),
        Version {
            major: 1,
            minor: 2,
            patch: 3
        }
    );
    assert_eq!(
        v("v0.10.0"),
        Version {
            major: 0,
            minor: 10,
            patch: 0
        }
    );
}

#[test]
fn parse_rejects_anything_but_major_minor_patch() {
    for bad in [
        "",
        "1.2",
        "1.2.3.4",
        "1.2.3-rc1",
        "a.b.c",
        "1..3",
        "+1.2.3",
        "1.2.-3",
        "vv1.2.3",
    ] {
        assert!(
            matches!(bad.parse::<Version>(), Err(UpdateError::InvalidVersion(_))),
            "expected {bad:?} to be rejected"
        );
    }
}

#[test]
fn ordering_is_numeric_not_lexical() {
    assert!(v("0.0.9") < v("0.0.10"));
    assert!(v("0.1.0") > v("0.0.99"));
    assert!(v("1.0.0") > v("0.99.99"));
    assert_eq!(v("2.3.4"), v("2.3.4"));
}

#[test]
fn compare_reports_up_to_date_available_and_newer() {
    assert_eq!(compare(v("0.0.1"), v("0.0.1")), UpdateStatus::UpToDate);
    assert_eq!(
        compare(v("0.0.1"), v("0.0.2")),
        UpdateStatus::UpdateAvailable
    );
    assert_eq!(
        compare(v("0.0.10"), v("0.0.9")),
        UpdateStatus::InstalledIsNewer
    );
}

#[test]
fn display_round_trips_through_parse() {
    let version = v("3.14.159");
    assert_eq!(version.to_string(), "3.14.159");
    assert_eq!(version.to_string().parse::<Version>().unwrap(), version);
}

#[test]
fn manifest_version_reads_the_version_field() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest.json");
    fs::write(&path, r#"{"version": "0.2.5", "channel": "stable"}"#).unwrap();
    assert_eq!(manifest_version(&path).unwrap(), v("0.2.5"));
}

#[test]
fn manifest_version_reports_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent.json");
    assert!(matches!(
        manifest_version(&path),
        Err(UpdateError::ManifestRead { .. })
    ));
}

#[test]
fn manifest_version_reports_bad_json_and_missing_field() {
    let dir = tempfile::tempdir().unwrap();
    let bad_json = dir.path().join("bad.json");
    fs::write(&bad_json, "{not json").unwrap();
    assert!(matches!(
        manifest_version(&bad_json),
        Err(UpdateError::ManifestParse { .. })
    ));

    let no_field = dir.path().join("no_field.json");
    fs::write(&no_field, r#"{"release": "0.2.5"}"#).unwrap();
    assert!(matches!(
        manifest_version(&no_field),
        Err(UpdateError::ManifestParse { .. })
    ));
}

#[test]
fn manifest_version_rejects_a_non_semver_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest.json");
    fs::write(&path, r#"{"version": "latest"}"#).unwrap();
    assert!(matches!(
        manifest_version(&path),
        Err(UpdateError::InvalidVersion(_))
    ));
}
