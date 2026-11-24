use super::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::connect_async;

/// Menjalankan server di port loopback acak dan mengembalikan alamatnya.
async fn start(server: Arc<Server>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(server, listener));
    format!("ws://{addr}")
}

async fn next_json<S>(ws: &mut tokio_tungstenite::WebSocketStream<S>) -> Value
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let message = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
        .await
        .expect("pesan harus datang dalam 5 detik")
        .expect("koneksi tidak boleh tertutup")
        .expect("pesan harus valid");
    match message {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("bukan pesan teks: {other:?}"),
    }
}

#[tokio::test]
async fn server_replays_missed_events_then_streams_live_ones() {
    let server = Server::new(16, 16);
    server.publish("doc", json!("satu"));
    server.publish("doc", json!("dua"));
    let url = start(Arc::clone(&server)).await;

    let (mut ws, _) = connect_async(&url).await.unwrap();
    ws.send(Message::Text(json!({ "last_seq": 0 }).to_string().into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut ws).await["seq"], 1);
    assert_eq!(next_json(&mut ws).await["seq"], 2);

    server.publish("doc", json!("tiga"));
    let live = next_json(&mut ws).await;
    assert_eq!(live["seq"], 3);
    assert_eq!(live["payload"], json!("tiga"));
}

#[tokio::test]
async fn server_client_resuming_after_seq_gets_only_newer_events() {
    let server = Server::new(16, 16);
    for n in 1..=3 {
        server.publish("doc", json!(n));
    }
    let url = start(Arc::clone(&server)).await;
    let (mut ws, _) = connect_async(&url).await.unwrap();
    ws.send(Message::Text(json!({ "last_seq": 2 }).to_string().into()))
        .await
        .unwrap();
    assert_eq!(next_json(&mut ws).await["seq"], 3);
}

#[tokio::test]
async fn server_reports_gap_when_replay_buffer_has_moved_on() {
    let server = Server::new(2, 16);
    for n in 1..=5 {
        server.publish("doc", json!(n));
    }
    let url = start(Arc::clone(&server)).await;
    let (mut ws, _) = connect_async(&url).await.unwrap();
    ws.send(Message::Text(json!({ "last_seq": 1 }).to_string().into()))
        .await
        .unwrap();
    let frame = next_json(&mut ws).await;
    assert_eq!(frame["gap"]["oldest_available"], 4);
}
