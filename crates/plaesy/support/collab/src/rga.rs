//! Teks replika berbasis RGA (Replicated Growable Array).
//!
//! Setiap karakter punya id `OpId` (jam Lamport lalu id replika). Sisip menyimpan
//! id karakter sebelumnya sebagai referensi, bukan indeks, sehingga operasi dari
//! replika lain tetap menunjuk tempat yang sama. Hapus hanya menandai karakter
//! sebagai batu nisan, jadi referensi tidak pernah hilang.
//!
//! Operasi yang dependensinya belum datang disimpan dulu di `pending` dan
//! diterapkan begitu dependensinya ada. Dengan begitu urutan pengiriman tidak
//! memengaruhi hasil akhir. Operasi yang sudah pernah diterapkan diabaikan.

use crate::error::CollabError;

/// Id peserta. Tidak boleh kosong.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplicaId(String);

impl ReplicaId {
    /// Membuat id peserta. Mengembalikan `EmptyReplicaId` jika teks kosong.
    pub fn new(id: impl Into<String>) -> Result<Self, CollabError> {
        let id = id.into();
        if id.is_empty() {
            return Err(CollabError::EmptyReplicaId);
        }
        Ok(Self(id))
    }

    /// Teks id peserta.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Id unik sebuah operasi sisip. Urutan lexicographic: jam dulu, lalu id replika.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpId {
    /// Jam Lamport saat operasi dibuat.
    pub lamport: u64,
    /// Peserta yang membuat operasi.
    pub replica: ReplicaId,
}

/// Operasi yang direplikasi antar peserta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Sisip `ch` tepat setelah karakter `after` (`None` berarti awal teks).
    Insert {
        id: OpId,
        after: Option<OpId>,
        ch: char,
    },
    /// Hapus karakter `target`. `lamport` adalah jam pembuat operasi.
    Delete { target: OpId, lamport: u64 },
}

#[derive(Debug, Clone)]
struct Node {
    id: OpId,
    ch: char,
    deleted: bool,
}

/// Satu replika catatan teks.
#[derive(Debug)]
pub struct Replica {
    id: ReplicaId,
    clock: u64,
    /// Semua karakter termasuk batu nisan, dalam urutan RGA.
    nodes: Vec<Node>,
    /// Operasi yang menunggu dependensi.
    pending: Vec<Op>,
    /// Operasi yang sudah diterapkan, dalam urutan kausal.
    history: Vec<Op>,
}

impl Replica {
    /// Replika kosong dengan jam nol.
    #[must_use]
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            clock: 0,
            nodes: Vec::new(),
            pending: Vec::new(),
            history: Vec::new(),
        }
    }

    /// Id peserta replika ini.
    #[must_use]
    pub fn id(&self) -> &ReplicaId {
        &self.id
    }

    /// Jam Lamport saat ini.
    #[must_use]
    pub fn clock(&self) -> u64 {
        self.clock
    }

    /// Jumlah karakter yang terlihat (tidak termasuk batu nisan).
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.iter().filter(|n| !n.deleted).count()
    }

    /// `true` jika tidak ada karakter yang terlihat.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Teks yang terlihat.
    #[must_use]
    pub fn text(&self) -> String {
        self.nodes
            .iter()
            .filter(|n| !n.deleted)
            .map(|n| n.ch)
            .collect()
    }

    /// Operasi yang sudah diterapkan, dalam urutan kausal.
    #[must_use]
    pub fn history(&self) -> &[Op] {
        &self.history
    }

    /// Jumlah operasi yang masih menunggu dependensi.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Sisip `ch` di indeks karakter yang terlihat (`0..=len`).
    /// Mengembalikan operasi untuk dikirim ke replika lain.
    pub fn insert_at(&mut self, index: usize, ch: char) -> Result<Op, CollabError> {
        let len = self.len();
        if index > len {
            return Err(CollabError::IndexOutOfRange { index, len });
        }
        let after = if index == 0 {
            None
        } else {
            Some(
                self.nth_visible(index - 1)
                    .ok_or(CollabError::IndexOutOfRange { index, len })?,
            )
        };
        let lamport = self.tick()?;
        let op = Op::Insert {
            id: OpId {
                lamport,
                replica: self.id.clone(),
            },
            after,
            ch,
        };
        self.integrate(&op)?;
        Ok(op)
    }

    /// Hapus karakter yang terlihat di indeks `index`.
    /// Mengembalikan operasi untuk dikirim ke replika lain.
    pub fn delete_at(&mut self, index: usize) -> Result<Op, CollabError> {
        let len = self.len();
        let target = self
            .nth_visible(index)
            .ok_or(CollabError::IndexOutOfRange { index, len })?;
        let lamport = self.tick()?;
        let op = Op::Delete { target, lamport };
        self.integrate(&op)?;
        Ok(op)
    }

    /// Terapkan operasi dari replika lain. Operasi duplikat diabaikan. Operasi
    /// yang dependensinya belum ada disimpan dan diterapkan nanti.
    pub fn apply(&mut self, op: Op) -> Result<(), CollabError> {
        validate(&op)?;
        if self.is_integrated(&op) || self.pending.contains(&op) {
            return Ok(());
        }
        self.pending.push(op);
        self.drain()
    }

    /// Terapkan seluruh riwayat dan operasi tertunda milik replika `other`.
    pub fn merge_from(&mut self, other: &Replica) -> Result<(), CollabError> {
        for op in other.history.iter().chain(other.pending.iter()) {
            self.apply(op.clone())?;
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<u64, CollabError> {
        self.clock = self
            .clock
            .checked_add(1)
            .ok_or(CollabError::ClockOverflow)?;
        Ok(self.clock)
    }

    fn observe(&mut self, lamport: u64) {
        if lamport > self.clock {
            self.clock = lamport;
        }
    }

    fn nth_visible(&self, index: usize) -> Option<OpId> {
        self.nodes
            .iter()
            .filter(|n| !n.deleted)
            .nth(index)
            .map(|n| n.id.clone())
    }

    fn position_of(&self, id: &OpId) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == *id)
    }

    /// Operasi sudah pernah diterapkan (sisip ada, atau hapus pada karakter
    /// yang sudah batu nisan).
    fn is_integrated(&self, op: &Op) -> bool {
        match op {
            Op::Insert { id, .. } => self.position_of(id).is_some(),
            Op::Delete { target, .. } => self
                .position_of(target)
                .is_some_and(|i| self.nodes[i].deleted),
        }
    }

    /// Dependensi operasi sudah ada di replika.
    fn is_ready(&self, op: &Op) -> bool {
        match op {
            Op::Insert { after, .. } => after
                .as_ref()
                .is_none_or(|a| self.position_of(a).is_some()),
            Op::Delete { target, .. } => self.position_of(target).is_some(),
        }
    }

    /// Terapkan operasi tertunda selama dependensinya sudah ada.
    fn drain(&mut self) -> Result<(), CollabError> {
        while let Some(i) = self.pending.iter().position(|op| self.is_ready(op)) {
            let op = self.pending.swap_remove(i);
            if !self.is_integrated(&op) {
                self.integrate(&op)?;
            }
        }
        Ok(())
    }

    /// Masukkan operasi yang dependensinya sudah ada ke `nodes`.
    fn integrate(&mut self, op: &Op) -> Result<(), CollabError> {
        match op {
            Op::Insert { id, after, ch } => {
                self.observe(id.lamport);
                let mut at = match after {
                    None => 0,
                    Some(a) => {
                        self.position_of(a)
                            .ok_or(CollabError::InvalidOp("induk sisip belum ada"))?
                            + 1
                    }
                };
                // Lewati karakter yang id-nya lebih besar: itu sisip konkuren
                // yang harus tampil lebih dulu. Aturan ini membuat urutan konvergen.
                while at < self.nodes.len() && self.nodes[at].id > *id {
                    at += 1;
                }
                self.nodes.insert(
                    at,
                    Node {
                        id: id.clone(),
                        ch: *ch,
                        deleted: false,
                    },
                );
            }
            Op::Delete { target, lamport } => {
                self.observe(*lamport);
                let i = self
                    .position_of(target)
                    .ok_or(CollabError::InvalidOp("hapus target belum ada"))?;
                self.nodes[i].deleted = true;
            }
        }
        self.history.push(op.clone());
        Ok(())
    }
}

/// Memeriksa bentuk operasi yang tidak bergantung pada keadaan replika.
fn validate(op: &Op) -> Result<(), CollabError> {
    match op {
        Op::Insert { id, after, .. } => {
            if id.lamport == 0 {
                return Err(CollabError::InvalidOp("jam logis nol"));
            }
            if after.as_ref().is_some_and(|a| a >= id) {
                return Err(CollabError::InvalidOp(
                    "induk harus lebih kecil dari id sisip",
                ));
            }
        }
        Op::Delete { lamport, .. } => {
            if *lamport == 0 {
                return Err(CollabError::InvalidOp("jam logis nol"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "rga_test.rs"]
mod rga_test;
