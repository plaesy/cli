use std::collections::VecDeque;

/// Keluaran yang belum dibaca dari sebuah sesi.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputChunk {
    /// Byte keluaran sejak pembacaan terakhir, urut dari yang paling lama.
    pub data: Vec<u8>,
    /// Byte yang terbuang karena buffer penuh sejak pembacaan terakhir.
    pub dropped_bytes: u64,
    /// `true` setelah pembaca keluaran mencapai akhir aliran. Di Windows (ConPTY)
    /// EOF sering baru datang setelah sesi ditutup, jadi jangan mengandalkannya
    /// sebagai tanda proses sudah keluar; gunakan `PtyManager::status`.
    pub closed: bool,
}

/// Buffer keluaran berbatas. Byte tertua dibuang saat kapasitas terlampaui.
#[derive(Debug)]
pub(crate) struct OutputBuffer {
    data: VecDeque<u8>,
    capacity: usize,
    dropped_bytes: u64,
    closed: bool,
}

impl OutputBuffer {
    /// `capacity` harus lebih besar dari nol; validasi dilakukan pemanggil.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::new(),
            capacity,
            dropped_bytes: 0,
            closed: false,
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) {
        if bytes.len() >= self.capacity {
            // Input saja sudah memenuhi buffer: buang seluruh isi lama dan
            // sisakan ekor input sepanjang kapasitas.
            let keep_from = bytes.len() - self.capacity;
            self.dropped_bytes += (self.data.len() + keep_from) as u64;
            self.data.clear();
            self.data.extend(&bytes[keep_from..]);
            return;
        }
        let overflow = (self.data.len() + bytes.len()).saturating_sub(self.capacity);
        self.data.drain(..overflow);
        self.dropped_bytes += overflow as u64;
        self.data.extend(bytes);
    }

    pub(crate) fn mark_closed(&mut self) {
        self.closed = true;
    }

    /// Mengambil seluruh isi buffer dan mengatur ulang penghitung byte terbuang.
    pub(crate) fn take(&mut self) -> OutputChunk {
        OutputChunk {
            data: self.data.drain(..).collect(),
            dropped_bytes: std::mem::take(&mut self.dropped_bytes),
            closed: self.closed,
        }
    }

    #[cfg(test)]
    pub(crate) fn buffered_len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
