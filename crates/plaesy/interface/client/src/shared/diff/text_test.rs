use super::*;

#[test]
fn text_identical_inputs_are_all_same() {
    let lines = diff_lines("a\nb", "a\nb");
    assert_eq!(lines, vec![Line::Same("a".into()), Line::Same("b".into())]);
}

#[test]
fn text_reports_added_and_removed_lines_in_order() {
    let lines = diff_lines("a\nb\nc", "a\nx\nc");
    assert_eq!(
        lines,
        vec![
            Line::Same("a".into()),
            Line::Removed("b".into()),
            Line::Added("x".into()),
            Line::Same("c".into()),
        ]
    );
}

#[test]
fn text_empty_side_is_all_added_or_removed() {
    assert_eq!(diff_lines("", "p"), vec![Line::Added("p".into())]);
    assert_eq!(diff_lines("p", ""), vec![Line::Removed("p".into())]);
}
