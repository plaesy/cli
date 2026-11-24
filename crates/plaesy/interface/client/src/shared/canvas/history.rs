//! Riwayat undo/redo untuk graf. Setiap commit menyimpan keadaan sebelumnya.

use super::graph::Graph;

#[derive(Debug, Clone, Default)]
pub struct History {
    current: Graph,
    undo: Vec<Graph>,
    redo: Vec<Graph>,
}

impl History {
    #[must_use]
    pub fn new(graph: Graph) -> Self {
        Self {
            current: graph,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    #[must_use]
    pub fn current(&self) -> &Graph {
        &self.current
    }

    /// Mengganti keadaan. Keadaan sebelumnya masuk riwayat undo, dan riwayat redo dikosongkan.
    pub fn commit(&mut self, next: Graph) {
        let previous = std::mem::replace(&mut self.current, next);
        self.undo.push(previous);
        self.redo.clear();
    }

    /// Kembali satu langkah. Mengembalikan `false` bila tidak ada yang bisa di-undo.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                let current = std::mem::replace(&mut self.current, previous);
                self.redo.push(current);
                true
            }
            None => false,
        }
    }

    /// Maju satu langkah. Mengembalikan `false` bila tidak ada yang bisa di-redo.
    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                let current = std::mem::replace(&mut self.current, next);
                self.undo.push(current);
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
#[path = "history_test.rs"]
mod history_test;
