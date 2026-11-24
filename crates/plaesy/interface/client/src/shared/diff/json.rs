//! Diff dokumen JSON menjadi daftar perubahan dengan path JSON Pointer (RFC 6901).
//! Objek dibandingkan per kunci, array per indeks, dan nilai lain per kesetaraan.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Added {
        path: String,
        value: Value,
    },
    Removed {
        path: String,
        value: Value,
    },
    Changed {
        path: String,
        from: Value,
        to: Value,
    },
}

impl Change {
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Added { path, .. } | Self::Removed { path, .. } | Self::Changed { path, .. } => {
                path
            }
        }
    }
}

/// Escape segmen JSON Pointer: `~` menjadi `~0`, `/` menjadi `~1`.
#[must_use]
pub fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Membandingkan `before` dan `after`. Hasil diurutkan menurut path agar deterministik.
#[must_use]
pub fn diff(before: &Value, after: &Value) -> Vec<Change> {
    let mut out = Vec::new();
    walk("", before, after, &mut out);
    out.sort_by(|a, b| a.path().cmp(b.path()));
    out
}

fn walk(path: &str, before: &Value, after: &Value, out: &mut Vec<Change>) {
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, av) in a {
                let child = format!("{path}/{}", escape(key));
                match b.get(key) {
                    Some(bv) => walk(&child, av, bv, out),
                    None => out.push(Change::Removed {
                        path: child,
                        value: av.clone(),
                    }),
                }
            }
            for (key, bv) in b {
                if !a.contains_key(key) {
                    out.push(Change::Added {
                        path: format!("{path}/{}", escape(key)),
                        value: bv.clone(),
                    });
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            let common = a.len().min(b.len());
            for index in 0..common {
                walk(&format!("{path}/{index}"), &a[index], &b[index], out);
            }
            for (index, av) in a.iter().enumerate().skip(common) {
                out.push(Change::Removed {
                    path: format!("{path}/{index}"),
                    value: av.clone(),
                });
            }
            for (index, bv) in b.iter().enumerate().skip(common) {
                out.push(Change::Added {
                    path: format!("{path}/{index}"),
                    value: bv.clone(),
                });
            }
        }
        _ => {
            if before != after {
                out.push(Change::Changed {
                    path: path.to_owned(),
                    from: before.clone(),
                    to: after.clone(),
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "json_test.rs"]
mod json_test;
