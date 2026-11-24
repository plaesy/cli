//! Antrean notifikasi untuk UI: tambah, tandai sudah dibaca, tutup, batasi jumlah, dan buang yang lama.
//!
//! Pure Rust tanpa I/O. Waktu dikirim pemanggil sebagai milidetik sejak epoch, sehingga tes dan
//! logika pembuangan tetap deterministik.

/// Tingkat notifikasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Informasi biasa.
    Info,
    /// Peringatan: perlu perhatian, tetapi belum gagal.
    Warning,
    /// Galat: suatu tindakan gagal.
    Error,
}

/// Satu notifikasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: u64,
    pub level: Level,
    pub message: String,
    /// Waktu dibuat dalam milidetik. Dipakai oleh [`Queue::prune_older_than`].
    pub created_at: u64,
    /// Benar bila notifikasi sudah ditandai dibaca.
    pub read: bool,
}

/// Antrean dengan batas jumlah. Yang paling lama dibuang saat penuh, termasuk yang belum dibaca.
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

    /// Batas jumlah notifikasi aktif.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Menambah notifikasi yang belum dibaca dan mengembalikan id-nya. `now` adalah waktu
    /// pembuatan dalam milidetik. Pesan kosong ditolak.
    ///
    /// # Errors
    ///
    /// [`NotificationError::EmptyMessage`] bila pesan kosong atau hanya spasi.
    pub fn push(
        &mut self,
        level: Level,
        message: &str,
        now: u64,
    ) -> Result<u64, NotificationError> {
        if message.trim().is_empty() {
            return Err(NotificationError::EmptyMessage);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Notification {
            id,
            level,
            message: message.to_owned(),
            created_at: now,
            read: false,
        });
        let excess = self.items.len().saturating_sub(self.capacity);
        self.items.drain(..excess);
        Ok(id)
    }

    /// Menutup notifikasi `id`. Mengembalikan `false` bila tidak ada.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.id != id);
        self.items.len() != before
    }

    /// Menandai notifikasi `id` sudah dibaca. Mengembalikan `false` bila tidak ada.
    /// Menandai yang sudah dibaca tidak berefek dan tetap mengembalikan `true`.
    pub fn mark_read(&mut self, id: u64) -> bool {
        match self.items.iter_mut().find(|n| n.id == id) {
            Some(item) => {
                item.read = true;
                true
            }
            None => false,
        }
    }

    /// Menandai semua notifikasi sudah dibaca.
    pub fn mark_all_read(&mut self) {
        for item in &mut self.items {
            item.read = true;
        }
    }

    /// Membuang notifikasi yang umurnya lebih dari `max_age` milidetik sejak `now`, baik yang
    /// sudah maupun belum dibaca. Notifikasi tepat berumur `max_age` masih disimpan. Bila `now`
    /// lebih kecil dari waktu pembuatan (jam mundur), umurnya dianggap nol. Mengembalikan jumlah
    /// yang dibuang.
    pub fn prune_older_than(&mut self, now: u64, max_age: u64) -> usize {
        let before = self.items.len();
        self.items.retain(|n| now.saturating_sub(n.created_at) <= max_age);
        before - self.items.len()
    }

    /// Semua notifikasi yang aktif, dari yang tertua.
    #[must_use]
    pub fn list(&self) -> &[Notification] {
        &self.items
    }

    /// Jumlah notifikasi yang belum dibaca.
    #[must_use]
    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|n| !n.read).count()
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
