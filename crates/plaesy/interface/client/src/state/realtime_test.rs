use super::*;

#[test]
fn realtime_sequence_numbers_are_monotonic_from_one() {
    let mut bus: Bus<u32> = Bus::new(10);
    assert_eq!(bus.last_seq(), 0);
    assert_eq!(bus.publish("doc", 1), 1);
    assert_eq!(bus.publish("doc", 2), 2);
    assert_eq!(bus.last_seq(), 2);
}

#[test]
fn realtime_subscriber_gets_only_new_events_and_advances() {
    let mut bus: Bus<u32> = Bus::new(10);
    bus.publish("doc", 1);
    let mut sub = Subscriber::new(None, 0);
    let first = sub.poll(&bus).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(sub.last_seq(), 1);
    assert!(sub.poll(&bus).unwrap().is_empty());
    bus.publish("doc", 2);
    let second = sub.poll(&bus).unwrap();
    assert_eq!(second[0].payload, 2);
}

#[test]
fn realtime_topic_filter_skips_other_topics_but_still_advances() {
    let mut bus: Bus<&str> = Bus::new(10);
    bus.publish("doc", "a");
    bus.publish("other", "b");
    let mut sub = Subscriber::new(Some("doc"), 0);
    let events = sub.poll(&bus).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload, "a");
    assert_eq!(sub.last_seq(), 2);
}

#[test]
fn realtime_overflow_is_reported_as_gap() {
    let mut bus: Bus<u32> = Bus::new(2);
    for n in 1..=5 {
        bus.publish("doc", n);
    }
    let mut stale = Subscriber::new(None, 1);
    assert_eq!(
        stale.poll(&bus),
        Err(PollError::Gap {
            oldest_available: 4
        })
    );
    let mut recent = Subscriber::new(None, 3);
    let events = recent.poll(&bus).unwrap();
    assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![4, 5]);
}

#[test]
fn realtime_empty_bus_yields_nothing_without_gap() {
    let bus: Bus<u32> = Bus::new(4);
    let mut sub = Subscriber::new(None, 0);
    assert!(sub.poll(&bus).unwrap().is_empty());
}
