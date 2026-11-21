//! Server WebSocket di atas [`Hub`]. Protokol di kabel:
//!
//! 1. Klien mengirim satu pesan teks `{"last_seq": N}` (0 bila belum pernah menerima).
//! 2. Server mengirim event setelah `N`, lalu event baru saat terbit. Setiap event berbentuk
//!    [`encode`]. Bila event yang diminta sudah keluar dari buffer, server mengirim
//!    `{"gap":{"oldest_available":M}}` lalu menutup koneksi agar klien memuat ulang keadaan.
//! 3. Bila klien tertinggal di siaran live (buffer siaran penuh), server juga mengirim gap
//!    dan menutup koneksi.
//!
//! Klien harus membuang event dengan `seq` yang sudah pernah diterima.

use crate::{encode, Envelope, Hub, Replay};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

/// Server realtime: menerbitkan event ke hub dan ke semua koneksi yang sedang aktif.
#[derive(Debug)]
pub struct Server {
    hub: Mutex<Hub>,
    live: broadcast::Sender<Envelope>,
}

impl Server {
    /// Server dengan buffer replay `capacity` event dan buffer siaran `live_capacity`.
    #[must_use]
    pub fn new(capacity: usize, live_capacity: usize) -> Arc<Self> {
        let (live, _) = broadcast::channel(live_capacity.max(1));
        Arc::new(Self {
            hub: Mutex::new(Hub::new(capacity)),
            live,
        })
    }

    /// Menerbitkan event. Mengembalikan nomor urutnya.
    ///
    /// # Panics
    ///
    /// Tidak pernah; kunci hub hanya dipegang sebentar dan tidak ada panik di dalamnya.
    pub fn publish(&self, topic: &str, payload: Value) -> u64 {
        let mut hub = self
            .hub
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let seq = hub.publish(topic, payload.clone());
        // Tidak ada penerima aktif bukan galat: event tetap ada di buffer replay.
        let _ = self.live.send(Envelope {
            seq,
            topic: topic.to_owned(),
            payload,
        });
        seq
    }

    /// Berlangganan dan menghitung replay dalam satu kunci, sehingga tidak ada event yang
    /// hilang atau terkirim dua kali di antara keduanya.
    fn subscribe_from(&self, last_seq: u64) -> (broadcast::Receiver<Envelope>, Replay) {
        let hub = self
            .hub
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (self.live.subscribe(), hub.replay_since(last_seq, None))
    }
}

/// Melayani koneksi dari `listener` sampai terjadi galat I/O pada accept.
///
/// # Errors
///
/// Galat I/O saat menerima koneksi.
pub async fn serve(server: Arc<Server>, listener: TcpListener) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            // Galat per koneksi hanya menutup koneksi itu.
            let _ = connection(server, stream).await;
        });
    }
}

async fn connection(
    server: Arc<Server>,
    stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;
    let last_seq = match ws.next().await {
        Some(Ok(Message::Text(text))) => serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| value.get("last_seq").and_then(Value::as_u64))
            .unwrap_or(0),
        _ => 0,
    };

    let (mut live, replay) = server.subscribe_from(last_seq);
    match replay {
        Replay::Gap { oldest_available } => {
            send_json(&mut ws, &gap(oldest_available)).await?;
            return Ok(());
        }
        Replay::Events(events) => {
            for event in &events {
                send_json(&mut ws, &encode(event)).await?;
            }
        }
    }

    loop {
        tokio::select! {
            incoming = ws.next() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            },
            event = live.recv() => match event {
                Ok(event) => send_json(&mut ws, &encode(&event)).await?,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Buffer siaran terlewati: klien harus memuat ulang keadaan penuh.
                    send_json(&mut ws, &gap(0)).await?;
                    break;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
    let _ = ws.close(None).await;
    Ok(())
}

fn gap(oldest_available: u64) -> Value {
    json!({ "gap": { "oldest_available": oldest_available } })
}

async fn send_json<S>(
    ws: &mut tokio_tungstenite::WebSocketStream<S>,
    value: &Value,
) -> Result<(), tokio_tungstenite::tungstenite::Error>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    ws.send(Message::Text(value.to_string().into())).await
}

#[cfg(test)]
#[path = "server_test.rs"]
mod server_test;
