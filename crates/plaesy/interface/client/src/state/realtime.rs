//! Bus event realtime di sisi klien (ADR 0011). Mirip `plaesy-interface-ws`: setiap event
//! punya nomor urut monoton. Pelanggan menyimpan `last_seq` dan mengambil event setelahnya.
//! Bila event yang dibutuhkan sudah keluar dari buffer, [`Subscriber::poll`] mengembalikan
//! [`PollError::Gap`] agar pelanggan memuat ulang keadaan. Tidak ada I/O, dan payload bersifat
//! generik sehingga bus tidak bergantung pada format JSON.

use std::collections::VecDeque;

/// Satu event dengan nomor urut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event<T> {
    pub seq: u64,
    pub topic: String,
    pub payload: T,
}

/// Bus dengan buffer terbatas.
#[derive(Debug, Clone)]
pub struct Bus<T> {
    capacity: usize,
    next_seq: u64,
    buffer: VecDeque<Event<T>>,
}

impl<T: Clone> Bus<T> {
    /// Bus yang menyimpan `capacity` event terakhir. Minimal 1.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_seq: 1,
            buffer: VecDeque::new(),
        }
    }

    /// Menerbitkan event dan mengembalikan nomor urutnya.
    pub fn publish(&mut self, topic: &str, payload: T) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.buffer.push_back(Event {
            seq,
            topic: topic.to_owned(),
            payload,
        });
        while self.buffer.len() > self.capacity {
            self.buffer.pop_front();
        }
        seq
    }

    /// Nomor urut terakhir yang diterbitkan, atau 0.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.next_seq - 1
    }

    fn oldest(&self) -> Option<u64> {
        self.buffer.front().map(|e| e.seq)
    }
}

/// Pelanggan: menyimpan posisi terakhir dan filter topik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscriber {
    topic: Option<String>,
    last_seq: u64,
}

/// Galat pelanggan saat mengambil event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollError {
    /// Event setelah `last_seq` sudah keluar dari buffer. Muat ulang keadaan penuh.
    Gap { oldest_available: u64 },
}

impl Subscriber {
    /// Pelanggan untuk satu topik, atau semua topik bila `None`. Mulai dari `since`.
    #[must_use]
    pub fn new(topic: Option<&str>, since: u64) -> Self {
        Self {
            topic: topic.map(str::to_owned),
            last_seq: since,
        }
    }

    /// Posisi terakhir yang sudah diambil.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Mengambil event baru sejak posisi terakhir, lalu memajukannya.
    ///
    /// # Errors
    ///
    /// [`PollError::Gap`] bila ada event yang hilang dari buffer.
    pub fn poll<T: Clone>(&mut self, bus: &Bus<T>) -> Result<Vec<Event<T>>, PollError> {
        if let Some(oldest) = bus.oldest() {
            if self.last_seq + 1 < oldest {
                return Err(PollError::Gap {
                    oldest_available: oldest,
                });
            }
        }
        let events: Vec<Event<T>> = bus
            .buffer
            .iter()
            .filter(|e| e.seq > self.last_seq)
            .filter(|e| self.topic.as_deref().is_none_or(|t| t == e.topic))
            .cloned()
            .collect();
        // Posisi maju ke event terakhir yang dilihat, termasuk yang tidak cocok topik.
        self.last_seq = bus.last_seq();
        Ok(events)
    }
}

#[cfg(test)]
#[path = "realtime_test.rs"]
mod realtime_test;
