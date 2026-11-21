//! Antrean notifikasi untuk UI: tambah, tutup, dan batasi jumlahnya.

/// Tingkat keparahan notifikasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

/// Satu notifikasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    pub level: Level,
    pub message: String,
}

/// Antrean dengan batas jumlah. Yang paling lama dibuang saat penuh.
#[derive(Debug, Clone)]
pub struct Queue {
    capacity: usize,
    next_id: u64,
    items: Vec<Notification>,
}

impl Queue {
    /// Antrean dengan kapasitas `capacity`. Minimal 1.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_id: 1,
            items: Vec::new(),
        }
    }

    /// Menambah notifikasi dan mengembalikan id-nya. Pesan kosong ditolak.
    ///
    /// # Errors
    ///
    /// [`NotificationError::EmptyMessage`] bila pesan kosong.
    pub fn push(&mut self, level: Level, message: &str) -> Result<u64, NotificationError> {
        if message.trim().is_empty() {
            return Err(NotificationError::EmptyMessage);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Notification {
            id,
            level,
            message: message.to_owned(),
        });
        if self.items.len() > self.capacity {
            self.items.remove(0);
        }
        Ok(id)
    }

    /// Menutup notifikasi `id`. Mengembalikan `false` bila tidak ada.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.id != id);
        self.items.len() != before
    }

    /// Semua notifikasi yang aktif, dari yang tertua.
    #[must_use]
    pub fn list(&self) -> &[Notification] {
        &self.items
    }

    /// Jumlah notifikasi aktif.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Benar bila tidak ada notifikasi.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Galat notifikasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    EmptyMessage,
}

#[cfg(test)]
#[path = "notifications_test.rs"]
mod notifications_test;
