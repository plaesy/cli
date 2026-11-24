use super::*;

#[test]
fn protocol_hello_carries_last_seq() {
    assert_eq!(hello(7), json!({ "last_seq": 7 }));
}

#[test]
fn protocol_parses_event_and_gap_frames() {
    let event = parse(&json!({"seq": 3, "topic": "doc", "payload": {"a": 1}})).unwrap();
    assert_eq!(
        event,
        Frame::Event(Event {
            seq: 3,
            topic: "doc".to_owned(),
            payload: json!({"a": 1})
        })
    );
    assert_eq!(
        parse(&json!({"gap": {"oldest_available": 4}})),
        Some(Frame::Gap {
            oldest_available: 4
        })
    );
}

#[test]
fn protocol_rejects_unknown_shapes() {
    assert_eq!(
        parse(&json!({"seq": "3", "topic": "doc", "payload": 1})),
        None
    );
    assert_eq!(parse(&json!({"hello": true})), None);
}

fn ev(seq: u64) -> Event {
    Event {
        seq,
        topic: "doc".to_owned(),
        payload: json!(seq),
    }
}

#[test]
fn protocol_tracker_accepts_in_order_events() {
    let mut tracker = Tracker::new(0);
    assert_eq!(tracker.accept(&ev(1)), Accepted::Fresh);
    assert_eq!(tracker.accept(&ev(2)), Accepted::Fresh);
    assert_eq!(tracker.last_seq(), 2);
}

#[test]
fn protocol_tracker_drops_duplicates() {
    let mut tracker = Tracker::new(5);
    assert_eq!(tracker.accept(&ev(3)), Accepted::Duplicate);
    assert_eq!(tracker.accept(&ev(5)), Accepted::Duplicate);
    assert_eq!(tracker.last_seq(), 5);
}

#[test]
fn protocol_tracker_reports_a_skipped_sequence() {
    let mut tracker = Tracker::new(2);
    assert_eq!(
        tracker.accept(&ev(5)),
        Accepted::Skipped {
            expected: 3,
            got: 5
        }
    );
    assert_eq!(tracker.last_seq(), 5);
}
