//! Measurements that gate Completion. A measurement that cannot be taken is UNKNOWN,
//! and an unknown value never counts as met.

use std::collections::BTreeMap;

/// Shown for a measurement with no command, or whose command gives no number.
pub const UNMEASURED_TEXT: &str = "UNKNOWN — not measured";

/// The value of one named measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    pub name: String,
    pub value: f64,
    /// False when the value could not be measured.
    pub known: bool,
    pub note: String,
}

/// Runs the declared command for `name` and reads its last output line as a number.
/// With no declared command, nothing runs and the result is unmeasured.
/// Runs one shell command, returning its exit code and output.
pub type CommandRun = dyn FnMut(&str) -> Result<(i32, String), String>;

pub fn measure(
    name: &str,
    declared: &BTreeMap<String, String>,
    run: &mut CommandRun,
) -> Measurement {
    let unknown = |note: String| Measurement {
        name: name.to_owned(),
        value: 0.0,
        known: false,
        note,
    };
    let Some(command) = declared.get(name).filter(|c| !c.trim().is_empty()) else {
        return unknown(UNMEASURED_TEXT.to_owned());
    };
    let (exit, out) = match run(command) {
        Ok(result) => result,
        Err(e) => return unknown(format!("UNKNOWN — command could not run: {e}")),
    };
    if exit != 0 {
        return unknown(format!("UNKNOWN — command exited {exit}"));
    }
    match last_line(&out).trim_end_matches('%').parse::<f64>() {
        Ok(value) => Measurement {
            name: name.to_owned(),
            value,
            known: true,
            note: "measured".into(),
        },
        Err(_) => unknown("UNKNOWN — output is not a number".into()),
    }
}

fn last_line(out: &str) -> &str {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
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
}
