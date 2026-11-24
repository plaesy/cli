use super::*;

#[test]
fn envelope_round_trips_and_plain_text_is_depth_zero() {
    let body = encode(2, "halo");
    assert_eq!(
        decode(&body),
        Envelope {
            depth: 2,
            text: "halo".to_owned()
        }
    );
    assert_eq!(
        decode("bukan json"),
        Envelope {
            depth: 0,
            text: "bukan json".to_owned()
        }
    );
}

#[test]
fn outgoing_text_and_depth_are_capped() {
    assert!(check_outgoing("halo", 1).is_ok());
    assert!(check_outgoing("   ", 1).is_err(), "empty text is refused");
    assert!(
        check_outgoing(&"x".repeat(MAX_TEXT + 1), 1).is_err(),
        "over the size cap"
    );
    assert!(check_outgoing("halo", MAX_DEPTH).is_ok());
    assert!(
        check_outgoing("halo", MAX_DEPTH + 1).is_err(),
        "past the depth cap"
    );
}

#[test]
fn id_is_stable_for_the_same_send_and_differs_otherwise() {
    let a = new_id("chat-a", "chat-b", "halo", 1);
    assert_eq!(
        a,
        new_id("chat-a", "chat-b", "halo", 1),
        "same input, same id (idempotent retry)"
    );
    assert_ne!(a, new_id("chat-a", "chat-b", "halo", 2));
    assert!(a.starts_with("a2a-") && a.len() == "a2a-".len() + 16);
}

#[test]
fn relay_arguments_match_the_relay_commands() {
    assert_eq!(
        send_args("chat-a", "chat-b", "a2a-1", "{}"),
        ["send", "--id", "a2a-1", "--from", "chat-a", "--to", "chat-b", "{}"]
    );
    assert_eq!(inbox_args("chat-b"), ["inbox", "--agent", "chat-b"]);
    assert_eq!(
        ack_args("a2a-1", "chat-b"),
        ["ack", "--id", "a2a-1", "--agent", "chat-b"]
    );
}

#[test]
fn inbox_json_parses_and_renders_with_depth() {
    let raw = r#"[{"id":"a2a-1","sender":"chat-a","recipient":"chat-b","body":"{\"depth\":1,\"text\":\"halo\"}","status":"menunggu"}]"#;
    let items = parse_inbox(raw).expect("valid");
    assert_eq!(items[0].sender, "chat-a");
    assert_eq!(render_incoming(&items), "a2a-1 from chat-a (depth 1): halo");
    assert!(parse_inbox("bukan json").is_err());
    assert_eq!(render_incoming(&[]), "no messages");
}
