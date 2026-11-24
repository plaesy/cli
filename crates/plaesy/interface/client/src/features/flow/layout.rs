//! Layout kanvas flow sebagai fungsi murni.
//!
//! Edge `(from, to)` berarti `from` mendahului `to` (aliran data dari kiri ke kanan).
//! Kolom sebuah modul adalah panjang jalur terpanjang dari sumber ke modul itu, sehingga
//! setiap edge selalu menuju kolom yang lebih kanan. Urutan baris dalam kolom dirapikan
//! dengan heuristik barycenter; ini mengurangi persilangan, tetapi tidak menjamin nol.
//!
//! Tidak ada I/O. Siklus dan input tidak valid dikembalikan sebagai `LayoutError`.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// Jarak antarkolom dalam satuan kanvas.
pub const COLUMN_GAP: i64 = 240;
/// Jarak antarbaris dalam satu kolom dalam satuan kanvas.
pub const ROW_GAP: i64 = 120;
/// Jumlah putaran sapuan barycenter bolak-balik.
const SWEEPS: usize = 4;

/// Posisi modul di kanvas. Hasil layout selalu bernilai non-negatif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

/// Galat layout. Setiap varian membawa id modul yang relevan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// Id modul muncul lebih dari sekali.
    DuplicateModule(String),
    /// Edge merujuk modul yang tidak ada dalam daftar.
    UnknownModule(String),
    /// Modul yang tidak bisa diurutkan topologis. Ini mencakup siklus dan modul hilir darinya.
    Cycle(Vec<String>),
    /// Posisi tujuan memiliki koordinat negatif.
    NegativePosition { x: i64, y: i64 },
    /// Modul yang akan dipindahkan tidak ada dalam peta posisi.
    MissingModule(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateModule(id) => write!(f, "modul ganda: {id}"),
            Self::UnknownModule(id) => write!(f, "edge merujuk modul tak dikenal: {id}"),
            Self::Cycle(ids) => write!(f, "siklus dependensi pada modul: {}", ids.join(", ")),
            Self::NegativePosition { x, y } => write!(f, "posisi negatif ditolak: ({x}, {y})"),
            Self::MissingModule(id) => write!(f, "modul tidak ditemukan: {id}"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Menghitung posisi setiap modul dengan layering topologis.
///
/// Mengembalikan `LayoutError::Cycle` jika graf tidak asiklik, dan tidak mengembalikan
/// posisi parsial. Daftar `modules` menentukan urutan awal dalam setiap kolom.
pub fn layout(
    modules: &[String],
    edges: &[(String, String)],
) -> Result<BTreeMap<String, Position>, LayoutError> {
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, id) in modules.iter().enumerate() {
        if index.insert(id.as_str(), i).is_some() {
            return Err(LayoutError::DuplicateModule(id.clone()));
        }
    }

    let n = modules.len();
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (from, to) in edges {
        let f = *index
            .get(from.as_str())
            .ok_or_else(|| LayoutError::UnknownModule(from.clone()))?;
        let t = *index
            .get(to.as_str())
            .ok_or_else(|| LayoutError::UnknownModule(to.clone()))?;
        succ[f].push(t);
        pred[t].push(f);
    }

    // Kahn: kolom = panjang jalur terpanjang dari sumber.
    let mut indeg: Vec<usize> = pred.iter().map(Vec::len).collect();
    let mut level = vec![0usize; n];
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut processed = 0usize;
    while let Some(u) = queue.pop_front() {
        processed += 1;
        for &v in &succ[u] {
            level[v] = level[v].max(level[u] + 1);
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push_back(v);
            }
        }
    }
    if processed < n {
        let rest = (0..n)
            .filter(|&i| indeg[i] > 0)
            .map(|i| modules[i].clone())
            .collect();
        return Err(LayoutError::Cycle(rest));
    }

    let depth = level.iter().copied().max().map_or(0, |d| d + 1);
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for (i, &l) in level.iter().enumerate() {
        columns[l].push(i);
    }

    let mut rank = vec![0usize; n];
    for column in &columns {
        refresh_rank(column, &mut rank);
    }
    for _ in 0..SWEEPS {
        for column in columns.iter_mut().skip(1) {
            sort_by_barycenter(column, &pred, &rank);
            refresh_rank(column, &mut rank);
        }
        for column in columns.iter_mut().rev().skip(1) {
            sort_by_barycenter(column, &succ, &rank);
            refresh_rank(column, &mut rank);
        }
    }

    let mut out = BTreeMap::new();
    for (i, id) in modules.iter().enumerate() {
        let position = Position {
            x: level[i] as i64 * COLUMN_GAP,
            y: rank[i] as i64 * ROW_GAP,
        };
        out.insert(id.clone(), position);
    }
    Ok(out)
}

/// Memindahkan satu modul ke `to`. Posisi negatif ditolak dan peta tidak berubah.
pub fn move_module(
    positions: &mut BTreeMap<String, Position>,
    id: &str,
    to: Position,
) -> Result<(), LayoutError> {
    if to.x < 0 || to.y < 0 {
        return Err(LayoutError::NegativePosition { x: to.x, y: to.y });
    }
    match positions.get_mut(id) {
        Some(slot) => {
            *slot = to;
            Ok(())
        }
        None => Err(LayoutError::MissingModule(id.to_owned())),
    }
}

/// Mengurutkan satu kolom berdasarkan rata-rata rank tetangga. Tanpa tetangga, rank sendiri
/// dipakai. Sort stabil menjaga urutan lama ketika nilainya sama.
fn sort_by_barycenter(column: &mut [usize], neighbors: &[Vec<usize>], rank: &[usize]) {
    let mut keyed: Vec<(f64, usize)> = column
        .iter()
        .map(|&i| {
            let adj = &neighbors[i];
            let key = if adj.is_empty() {
                rank[i] as f64
            } else {
                adj.iter().map(|&j| rank[j] as f64).sum::<f64>() / adj.len() as f64
            };
            (key, i)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (slot, (_, i)) in column.iter_mut().zip(keyed) {
        *slot = i;
    }
}

fn refresh_rank(column: &[usize], rank: &mut [usize]) {
    for (r, &i) in column.iter().enumerate() {
        rank[i] = r;
    }
}

#[cfg(test)]
#[path = "layout_test.rs"]
mod layout_test;
