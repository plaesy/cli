use super::*;

fn ev(name: &str, data: &str) -> Event {
    Event {
        name: name.to_owned(),
        data: data.to_owned(),
    }
}

#[test]
fn frames_single_line_event() {
    assert_eq!(
        frame(&ev("task", "ok")).unwrap(),
        "event: task\ndata: ok\n\n"
    );
}

#[test]
fn splits_multi_line_data_into_data_lines() {
    assert_eq!(
        frame(&ev("log", "a\nb\r\nc")).unwrap(),
        "event: log\ndata: a\ndata: b\ndata: c\n\n"
    );
}

#[test]
fn empty_data_is_one_empty_data_line() {
    assert_eq!(frame(&ev("ping", "")).unwrap(), "event: ping\ndata: \n\n");
}

#[test]
fn rejects_empty_name() {
    assert_eq!(frame(&ev("", "x")), Err(SseError::EmptyName));
}

#[test]
fn rejects_line_break_in_name_so_no_second_event_can_be_injected() {
    assert_eq!(
        frame(&ev("a\ndata: injected", "x")),
        Err(SseError::NameLineBreak)
    );
    assert_eq!(frame(&ev("a\rb", "x")), Err(SseError::NameLineBreak));
}
