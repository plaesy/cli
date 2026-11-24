//! Graf berarah: node dengan posisi, edge antar node, dan seleksi.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    DuplicateNode(String),
    UnknownNode(String),
    SelfLoop(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graph {
    nodes: BTreeMap<String, Node>,
    edges: Vec<Edge>,
}

impl Graph {
    /// Menambah node. Id harus unik.
    ///
    /// # Errors
    ///
    /// [`GraphError::DuplicateNode`] bila id sudah ada.
    pub fn add_node(&mut self, node: Node) -> Result<(), GraphError> {
        if self.nodes.contains_key(&node.id) {
            return Err(GraphError::DuplicateNode(node.id));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Menghubungkan dua node. Edge yang sama tidak ditambahkan dua kali.
    ///
    /// # Errors
    ///
    /// Node tidak dikenal, atau edge ke diri sendiri.
    pub fn connect(&mut self, from: &str, to: &str) -> Result<(), GraphError> {
        for id in [from, to] {
            if !self.nodes.contains_key(id) {
                return Err(GraphError::UnknownNode(id.to_owned()));
            }
        }
        if from == to {
            return Err(GraphError::SelfLoop(from.to_owned()));
        }
        let edge = Edge {
            from: from.to_owned(),
            to: to.to_owned(),
        };
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
        Ok(())
    }

    /// Menghapus node beserta edge yang menyentuhnya.
    ///
    /// # Errors
    ///
    /// Node tidak dikenal.
    pub fn remove_node(&mut self, id: &str) -> Result<(), GraphError> {
        if self.nodes.remove(id).is_none() {
            return Err(GraphError::UnknownNode(id.to_owned()));
        }
        self.edges.retain(|e| e.from != id && e.to != id);
        Ok(())
    }

    /// Memindahkan node. Mengembalikan `false` bila node tidak ada.
    pub fn move_node(&mut self, id: &str, x: f64, y: f64) -> bool {
        match self.nodes.get_mut(id) {
            Some(node) => {
                node.x = x;
                node.y = y;
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
}

/// Seleksi node. Urutannya tidak berarti apa-apa; yang penting keanggotaannya.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection(BTreeSet<String>);

impl Selection {
    pub fn select(&mut self, id: &str) {
        self.0.insert(id.to_owned());
    }

    /// Membalik keanggotaan. Mengembalikan `true` bila sekarang terpilih.
    pub fn toggle(&mut self, id: &str) -> bool {
        if self.0.remove(id) {
            false
        } else {
            self.0.insert(id.to_owned());
            true
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

#[cfg(test)]
#[path = "graph_test.rs"]
mod graph_test;
