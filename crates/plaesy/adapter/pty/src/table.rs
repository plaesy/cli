use std::collections::BTreeMap;
use std::fmt;

use crate::error::PtyError;

/// Pengenal sesi. Nilainya naik monoton dan tidak dipakai ulang dalam satu manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(u64);

impl SessionId {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Tabel sesi dengan batas jumlah entri. Generik agar logika batas bisa diuji
/// tanpa membuat proses terminal.
#[derive(Debug)]
pub(crate) struct SessionTable<T> {
    limit: usize,
    next_id: u64,
    entries: BTreeMap<SessionId, T>,
}

impl<T> SessionTable<T> {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit,
            next_id: 1,
            entries: BTreeMap::new(),
        }
    }

    pub(crate) fn has_capacity(&self) -> bool {
        self.entries.len() < self.limit
    }

    pub(crate) fn insert(&mut self, value: T) -> Result<SessionId, PtyError> {
        if !self.has_capacity() {
            return Err(PtyError::SessionLimitReached { limit: self.limit });
        }
        let id = SessionId(self.next_id);
        self.next_id += 1;
        self.entries.insert(id, value);
        Ok(id)
    }

    pub(crate) fn get_mut(&mut self, id: SessionId) -> Result<&mut T, PtyError> {
        self.entries
            .get_mut(&id)
            .ok_or(PtyError::SessionNotFound(id))
    }

    pub(crate) fn remove(&mut self, id: SessionId) -> Result<T, PtyError> {
        self.entries
            .remove(&id)
            .ok_or(PtyError::SessionNotFound(id))
    }
}

#[cfg(test)]
#[path = "table_test.rs"]
mod table_test;
