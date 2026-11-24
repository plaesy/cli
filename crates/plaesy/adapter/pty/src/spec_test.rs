use super::*;

#[test]
fn plain_command_is_valid() {
    let mut spec = CommandSpec::new("sh");
    spec.args = vec!["-c".to_owned(), "echo hi".to_owned()];
    spec.cwd = Some(PathBuf::from("work"));
    assert!(spec.validate().is_ok());
}

#[test]
fn empty_or_blank_program_is_rejected() {
    for program in ["", "   ", "\t"] {
        let err = CommandSpec::new(program).validate().unwrap_err();
        assert!(
            matches!(err, PtyError::InvalidCommand("program is empty")),
            "program {program:?} gave {err:?}"
        );
    }
}

#[test]
fn nul_byte_in_program_or_argument_is_rejected() {
    let err = CommandSpec::new("sh\0x").validate().unwrap_err();
    assert!(matches!(err, PtyError::InvalidCommand(_)), "{err:?}");

    let mut spec = CommandSpec::new("sh");
    spec.args = vec!["ok".to_owned(), "bad\0arg".to_owned()];
    let err = spec.validate().unwrap_err();
    assert!(matches!(err, PtyError::InvalidCommand(_)), "{err:?}");
}

#[test]
fn empty_or_nul_working_directory_is_rejected() {
    let mut spec = CommandSpec::new("sh");
    spec.cwd = Some(PathBuf::new());
    assert!(matches!(
        spec.validate().unwrap_err(),
        PtyError::InvalidCommand("working directory is empty")
    ));

    spec.cwd = Some(PathBuf::from("a\0b"));
    assert!(matches!(
        spec.validate().unwrap_err(),
        PtyError::InvalidCommand(_)
    ));
}

#[test]
fn default_term_size_is_valid() {
    assert!(TermSize::default().validate().is_ok());
}

#[test]
fn term_size_bounds_are_inclusive() {
    let max = MAX_TERMINAL_DIMENSION;
    assert!(TermSize { rows: 1, cols: 1 }.validate().is_ok());
    assert!(TermSize { rows: max, cols: max }.validate().is_ok());
}

#[test]
fn zero_or_oversized_term_size_is_rejected() {
    let max = MAX_TERMINAL_DIMENSION;
    let cases = [
        TermSize { rows: 0, cols: 80 },
        TermSize { rows: 24, cols: 0 },
        TermSize { rows: max + 1, cols: 80 },
        TermSize { rows: 24, cols: u16::MAX },
    ];
    for size in cases {
        assert!(
            matches!(size.validate(), Err(PtyError::InvalidSize { .. })),
            "{size:?} was accepted"
        );
    }
}

#[test]
fn to_pty_size_carries_rows_and_cols() {
    let size = TermSize { rows: 40, cols: 120 }.to_pty_size();
    assert_eq!((size.rows, size.cols), (40, 120));
    assert_eq!((size.pixel_width, size.pixel_height), (0, 0));
}
