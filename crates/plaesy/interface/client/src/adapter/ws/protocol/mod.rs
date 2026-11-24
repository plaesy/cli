//! Protokol realtime sisi klien (ADR 0011). Murni: tidak ada I/O.
//!
//! Klien mengirim `{"last_seq": N}`. Server membalas event setelah N, atau `{"gap": ...}`
//! bila event itu sudah keluar dari buffer. [`Tracker`] membuang duplikat dan melaporkan
//! lompatan nomor urut.

use serde_json::{json, Value};

/// Satu event dari server.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub seq: u64,
    pub topic: String,
    pub payload: Value,
}

/// Bentuk pesan yang diterima dari server.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Event(Event),
    /// Server menyatakan event sudah tidak tersedia. Klien harus memuat ulang keadaan penuh.
    Gap {
        oldest_available: u64,
    },
}

/// Hasil penerimaan satu event oleh [`Tracker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accepted {
    /// Event baru dan berurutan.
    Fresh,
    /// Nomor urut sudah pernah diterima; dibuang.
    Duplicate,
    /// Ada lompatan: nomor berikutnya yang diharapkan dan yang datang.
    Skipped { expected: u64, got: u64 },
}

/// Pesan hello untuk `last_seq`.
#[must_use]
pub fn hello(last_seq: u64) -> Value {
    json!({ "last_seq": last_seq })
}

/// Menerjemahkan satu pesan dari server. `None` bila bentuknya tidak dikenal.
#[must_use]
pub fn parse(value: &Value) -> Option<Frame> {
    if let Some(gap) = value.get("gap") {
        return Some(Frame::Gap {
            oldest_available: gap.get("oldest_available")?.as_u64()?,
        });
    }
    Some(Frame::Event(Event {
        seq: value.get("seq")?.as_u64()?,
        topic: value.get("topic")?.as_str()?.to_owned(),
        payload: value.get("payload")?.clone(),
    }))
}

/// Melacak nomor urut terakhir yang diterima.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tracker {
    last_seq: u64,
}

impl Tracker {
    /// Mulai dari `last_seq` yang sudah dimiliki klien.
    #[must_use]
    pub fn new(last_seq: u64) -> Self {
        Self { last_seq }
    }

    /// Nomor urut terakhir yang diterima.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Mencatat satu event.
    pub fn accept(&mut self, event: &Event) -> Accepted {
        if event.seq <= self.last_seq {
            return Accepted::Duplicate;
        }
        let expected = self.last_seq + 1;
        self.last_seq = event.seq;
        if event.seq == expected {
            Accepted::Fresh
        } else {
            Accepted::Skipped {
                expected,
                got: event.seq,
            }
        }
    }
}

#[cfg(test)]
#[path = "protocol_test.rs"]
mod protocol_test;
