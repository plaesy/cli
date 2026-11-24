//! Diff teks per baris memakai LCS. Memakai O(n·m) memori; cocok untuk berkas kecil dan sedang.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Same(String),
    Added(String),
    Removed(String),
}

/// Diff dua teks per baris, dengan urutan asli dipertahankan.
#[must_use]
pub fn diff_lines(before: &str, after: &str) -> Vec<Line> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let (n, m) = (a.len(), b.len());

    // lcs[i][j] = panjang LCS dari a[i..] dan b[j..].
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(Line::Same(a[i].to_owned()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(Line::Removed(a[i].to_owned()));
            i += 1;
        } else {
            out.push(Line::Added(b[j].to_owned()));
            j += 1;
        }
    }
    for line in &a[i..] {
        out.push(Line::Removed((*line).to_owned()));
    }
    for line in &b[j..] {
        out.push(Line::Added((*line).to_owned()));
    }
    out
}

#[cfg(test)]
#[path = "text_test.rs"]
mod text_test;
