use super::*;
use serde_json::json;

#[test]
fn ws_sequence_numbers_are_monotonic_from_one() {
    let mut hub = Hub::new(10);
    assert_eq!(hub.last_seq(), 0);
    assert_eq!(hub.publish("doc", json!(1)), 1);
    assert_eq!(hub.publish("doc", json!(2)), 2);
    assert_eq!(hub.last_seq(), 2);
}

#[test]
fn ws_replay_returns_only_events_after_last_seen() {
    let mut hub = Hub::new(10);
    for n in 1..=4 {
        hub.publish("doc", json!(n));
    }
    let Replay::Events(events) = hub.replay_since(2, None) else {
        panic!("tidak boleh gap");
    };
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![3, 4]);
}

#[test]
fn ws_replay_filters_by_topic() {
    let mut hub = Hub::new(10);
    hub.publish("doc", json!("a"));
    hub.publish("other", json!("b"));
    hub.publish("doc", json!("c"));
    let Replay::Events(events) = hub.replay_since(0, Some("doc")) else {
        panic!("tidak boleh gap");
    };
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e.topic == "doc"));
}

#[test]
fn ws_buffer_overflow_is_reported_as_gap_not_silent_loss() {
    let mut hub = Hub::new(2);
    for n in 1..=5 {
        hub.publish("doc", json!(n));
    }
    // Buffer hanya menyimpan seq 4 dan 5. Klien yang berhenti di 1 tertinggal.
    assert_eq!(
        hub.replay_since(1, None),
        Replay::Gap {
            oldest_available: 4
        }
    );
    // Klien yang berhenti di 3 masih bisa disusul penuh.
    let Replay::Events(events) = hub.replay_since(3, None) else {
        panic!("seq 4 masih ada di buffer");
    };
    assert_eq!(events.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![4, 5]);
}

#[test]
fn ws_empty_hub_replays_nothing() {
    let hub = Hub::new(4);
    assert_eq!(hub.replay_since(0, None), Replay::Events(vec![]));
}

#[test]
fn ws_encode_and_decode_roundtrip() {
    let envelope = Envelope {
        seq: 7,
        topic: "doc".to_owned(),
        payload: json!({"k": "v"}),
    };
    assert_eq!(decode(&encode(&envelope)), Some(envelope));
}

#[test]
fn ws_decode_rejects_malformed_frames() {
    assert_eq!(
        decode(&json!({"seq": "7", "topic": "doc", "payload": 1})),
        None
    );
    assert_eq!(decode(&json!({"topic": "doc", "payload": 1})), None);
}
