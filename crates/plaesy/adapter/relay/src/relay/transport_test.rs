use super::*;
use crate::relay::auth::seal;
use crate::relay::keys::StaticKeySource;
use std::net::Ipv4Addr;

fn service() -> Service<StaticKeySource> {
    let mut keys = StaticKeySource::new();
    keys.insert("k1", b"deployment-key");
    Service::new(keys, 300)
}

fn envelope(nonce: &str, sent_at: u64) -> String {
    seal(
        b"deployment-key",
        "k1",
        "agent-1",
        "project-a",
        "agent-2",
        sent_at,
        nonce,
        "hello",
    )
}

#[test]
fn shared_messaging_transport_accepts_once_then_reports_replay() {
    let svc = service();
    let env = envelope("n-1", 1000);
    assert_eq!(handle(&svc, &env, 1000).0, 202);
    assert_eq!(handle(&svc, &env, 1001).0, 409);
    assert_eq!(svc.effects(), 1);
}

#[test]
fn shared_messaging_transport_rejects_unknown_key_bad_tag_and_expired() {
    let svc = service();
    let other_key = seal(
        b"other",
        "k1",
        "agent-1",
        "project-a",
        "agent-2",
        1000,
        "n-2",
        "hi",
    );
    assert_eq!(handle(&svc, &other_key, 1000).0, 401);
    let unknown = seal(
        b"deployment-key",
        "k7",
        "agent-1",
        "project-a",
        "agent-2",
        1000,
        "n-3",
        "hi",
    );
    assert_eq!(handle(&svc, &unknown, 1000).0, 401);
    let stale = envelope("n-4", 1000);
    assert_eq!(handle(&svc, &stale, 1000 + 301).0, 401);
    assert_eq!(handle(&svc, "not an envelope", 1000).0, 400);
}

#[test]
fn shared_messaging_transport_refuses_public_bind_without_upstream_tls() {
    let err = bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)), false).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn shared_messaging_transport_round_trip_over_loopback() {
    let svc = Arc::new(service());
    let listener = bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), false).unwrap();
    let addr = listener.local_addr().unwrap();
    let server_svc = Arc::clone(&svc);
    let handle_thread = std::thread::spawn(move || serve(&listener, &server_svc, Some(1)));
    let (code, body) = post(addr, &envelope("n-5", now_secs())).unwrap();
    handle_thread.join().unwrap().unwrap();
    assert_eq!(code, 202);
    assert!(body.contains("Queued"));
    assert_eq!(svc.effects(), 1);
}

#[test]
fn shared_messaging_transport_key_rotation_overlap_then_retire() {
    let mut keys = StaticKeySource::new();
    keys.insert("k1", b"deployment-key");
    keys.insert("k2", b"rotated-key");
    let svc = Service::new(keys, 300);
    let old = envelope("n-rot-1", 1000);
    let new = seal(
        b"rotated-key",
        "k2",
        "agent-1",
        "project-a",
        "agent-2",
        1000,
        "n-rot-2",
        "hi",
    );
    assert_eq!(handle(&svc, &old, 1000).0, 202);
    assert_eq!(handle(&svc, &new, 1000).0, 202);
    let mut retired = StaticKeySource::new();
    retired.insert("k2", b"rotated-key");
    let svc2 = Service::new(retired, 300);
    assert_eq!(handle(&svc2, &old, 1000).0, 401);
    assert_eq!(handle(&svc2, &new, 1000).0, 202);
}
