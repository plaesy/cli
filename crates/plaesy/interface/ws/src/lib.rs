//! Inti protokol realtime (ADR 0011, bagian `adapter/ws/protocol`): envelope bernomor urut,
//! buffer replay terbatas, dan langganan. Tidak ada I/O di sini. Binding socket (WebSocket)
//! ada di modul `server`, memakai tokio-tungstenite.
//!
//! Klien mengirim `last_seq` terakhir yang diterimanya. Hub mengembalikan event setelah nomor
//! itu. Bila event yang diminta sudah keluar dari buffer, hub menyatakannya eksplisit
//! (`Replay::Gap`) agar klien memuat ulang keadaan penuh, tidak diam-diam kehilangan data.

pub mod server;

use serde_json::Value;
use std::collections::VecDeque;

/// Satu event dengan nomor urut monoton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub seq: u64,
    pub topic: String,
    pub payload: Value,
}

/// Hasil replay untuk klien yang tertinggal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replay {
    /// Semua event setelah `last_seq`, dalam urutan.
    Events(Vec<Envelope>),
    /// Event tertua di buffer sudah lebih baru dari `last_seq + 1`; klien harus memuat ulang.
    Gap { oldest_available: u64 },
}

/// Hub event dengan buffer replay berkapasitas tetap.
#[derive(Debug)]
pub struct Hub {
    capacity: usize,
    next_seq: u64,
    buffer: VecDeque<Envelope>,
}

impl Hub {
    /// Membuat hub yang menyimpan `capacity` event terakhir untuk replay. Minimal 1.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_seq: 1,
            buffer: VecDeque::new(),
        }
    }

    /// Menerbitkan event dan mengembalikan nomor urutnya.
    pub fn publish(&mut self, topic: &str, payload: Value) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.buffer.push_back(Envelope {
            seq,
            topic: topic.to_owned(),
            payload,
        });
        while self.buffer.len() > self.capacity {
            self.buffer.pop_front();
        }
        seq
    }

    /// Nomor urut terakhir yang diterbitkan, atau 0 bila belum ada.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.next_seq - 1
    }

    /// Event setelah `last_seq`, atau [`Replay::Gap`] bila sebagiannya sudah keluar dari buffer.
    /// Filter `topic` bila diberikan. Gap dihitung terhadap semua topik.
    #[must_use]
    pub fn replay_since(&self, last_seq: u64, topic: Option<&str>) -> Replay {
        let oldest = self.buffer.front().map(|event| event.seq);
        match oldest {
            Some(oldest_seq) if last_seq + 1 < oldest_seq => Replay::Gap {
                oldest_available: oldest_seq,
            },
            _ => Replay::Events(
                self.buffer
                    .iter()
                    .filter(|event| event.seq > last_seq)
                    .filter(|event| topic.is_none_or(|t| event.topic == t))
                    .cloned()
                    .collect(),
            ),
        }
    }
}

/// Bentuk pesan di kabel: `{"seq":N,"topic":"...","payload":...}`.
#[must_use]
pub fn encode(envelope: &Envelope) -> Value {
    serde_json::json!({
        "seq": envelope.seq,
        "topic": envelope.topic,
        "payload": envelope.payload,
    })
}

/// Membaca bentuk di kabel. `None` bila bentuk tidak sesuai.
#[must_use]
pub fn decode(value: &Value) -> Option<Envelope> {
    Some(Envelope {
        seq: value.get("seq")?.as_u64()?,
        topic: value.get("topic")?.as_str()?.to_owned(),
        payload: value.get("payload")?.clone(),
    })
}

#[cfg(test)]
#[path = "ws_test.rs"]
mod ws_test;
