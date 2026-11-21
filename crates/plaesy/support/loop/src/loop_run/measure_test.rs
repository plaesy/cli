//! Unit tests for `tests` split out of `measure.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn undeclared_measurement_is_unknown_and_runs_nothing() {
    let m = measure("coverage", &BTreeMap::new(), &mut |_| {
        panic!("must not run")
    });
    assert!(!m.known);
    assert_eq!(m.note, UNMEASURED_TEXT);
}

#[test]
fn last_line_is_read_as_a_percentage() {
    let declared = BTreeMap::from([("coverage".to_owned(), "cov".to_owned())]);
    let m = measure("coverage", &declared, &mut |_| {
        Ok((0, "running\n93.5%\n".into()))
    });
    assert!(m.known);
    assert_eq!(m.value, 93.5);
}

#[test]
fn non_numeric_output_is_unknown() {
    let declared = BTreeMap::from([("coverage".to_owned(), "cov".to_owned())]);
    let m = measure("coverage", &declared, &mut |_| Ok((0, "all good".into())));
    assert!(!m.known);
}
