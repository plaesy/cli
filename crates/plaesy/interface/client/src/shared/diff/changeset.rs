//! Changeset: kumpulan perubahan yang bisa digabung dari dua sumber. Dua perubahan bertentangan
//! bila path-nya sama, atau salah satu path adalah induk dari yang lain, dan isinya berbeda.

use super::json::Change;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Changeset {
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
}

/// Dua path tumpang tindih bila sama, atau salah satunya induk (diakhiri `/`) dari yang lain.
fn overlaps(a: &str, b: &str) -> bool {
    a == b || b.starts_with(&format!("{a}/")) || a.starts_with(&format!("{b}/"))
}

impl Changeset {
    #[must_use]
    pub fn from_diff(changes: Vec<Change>) -> Self {
        Self { changes }
    }

    /// Menggabungkan `other` ke changeset ini. Perubahan identik tidak dihitung konflik.
    ///
    /// # Errors
    ///
    /// [`Conflict`] dengan path pertama yang bertentangan.
    pub fn merge(&self, other: &Changeset) -> Result<Changeset, Conflict> {
        let mut out = self.changes.clone();
        for change in &other.changes {
            let clash = out
                .iter()
                .find(|existing| overlaps(existing.path(), change.path()) && *existing != change);
            if let Some(existing) = clash {
                let path = if existing.path().len() >= change.path().len() {
                    existing.path().to_owned()
                } else {
                    change.path().to_owned()
                };
                return Err(Conflict { path });
            }
            if !out.contains(change) {
                out.push(change.clone());
            }
        }
        out.sort_by(|a, b| a.path().cmp(b.path()));
        Ok(Changeset { changes: out })
    }
}

#[cfg(test)]
#[path = "changeset_test.rs"]
mod changeset_test;
