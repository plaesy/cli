//! Reducer editor flow: aksi murni atas daftar modul.

use super::model::ModuleView;

/// Aksi yang bisa dilakukan pengguna di editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    AddModule(ModuleView),
    RemoveModule(String),
    RenameModule {
        from: String,
        to: String,
    },
    Connect {
        to: String,
        input: String,
        source: String,
    },
}

/// Galat aksi yang tidak bisa dijalankan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    NotFound(String),
    AlreadyExists(String),
}

/// Menerapkan `action` ke `modules` dan mengembalikan daftar baru. Daftar lama tidak diubah.
///
/// # Errors
///
/// `NotFound` bila modul tidak ada, `AlreadyExists` bila id bentrok.
pub fn apply(modules: &[ModuleView], action: &Action) -> Result<Vec<ModuleView>, ActionError> {
    let mut next = modules.to_vec();
    match action {
        Action::AddModule(module) => {
            if next.iter().any(|m| m.id == module.id) {
                return Err(ActionError::AlreadyExists(module.id.clone()));
            }
            next.push(module.clone());
        }
        Action::RemoveModule(id) => {
            let before = next.len();
            next.retain(|m| &m.id != id);
            if next.len() == before {
                return Err(ActionError::NotFound(id.clone()));
            }
            // Rujukan yang tersisa dibiarkan; validasi akan menandainya.
        }
        Action::RenameModule { from, to } => {
            if next.iter().any(|m| &m.id == to) {
                return Err(ActionError::AlreadyExists(to.clone()));
            }
            let module = next
                .iter_mut()
                .find(|m| &m.id == from)
                .ok_or_else(|| ActionError::NotFound(from.clone()))?;
            module.id = to.clone();
            for other in &mut next {
                for (_, expr) in &mut other.inputs {
                    *expr = rename_reference(expr, from, to);
                }
            }
        }
        Action::Connect { to, input, source } => {
            let module = next
                .iter_mut()
                .find(|m| &m.id == to)
                .ok_or_else(|| ActionError::NotFound(to.clone()))?;
            match module.inputs.iter_mut().find(|(name, _)| name == input) {
                Some((_, expr)) => *expr = source.clone(),
                None => module.inputs.push((input.clone(), source.clone())),
            }
        }
    }
    Ok(next)
}

fn rename_reference(expr: &str, from: &str, to: &str) -> String {
    match expr.split_once('.') {
        Some((head, rest)) if head == from => format!("{to}.{rest}"),
        None if expr == from => to.to_owned(),
        _ => expr.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(id: &str) -> ModuleView {
        ModuleView {
            id: id.to_owned(),
            kind: "action".to_owned(),
            operation: String::new(),
            inputs: Vec::new(),
        }
    }

    #[test]
    fn add_then_duplicate_is_refused() {
        let modules = apply(&[], &Action::AddModule(module("m1"))).unwrap();
        assert_eq!(
            apply(&modules, &Action::AddModule(module("m1"))),
            Err(ActionError::AlreadyExists("m1".into()))
        );
    }

    #[test]
    fn remove_missing_module_is_not_found() {
        assert_eq!(
            apply(&[], &Action::RemoveModule("m9".into())),
            Err(ActionError::NotFound("m9".into()))
        );
    }

    #[test]
    fn connect_sets_or_adds_an_input() {
        let base = vec![module("m1"), module("m2")];
        let modules = apply(
            &base,
            &Action::Connect {
                to: "m2".into(),
                input: "x".into(),
                source: "m1.out".into(),
            },
        )
        .unwrap();
        assert_eq!(
            modules[1].inputs,
            vec![("x".to_owned(), "m1.out".to_owned())]
        );
        let again = apply(
            &modules,
            &Action::Connect {
                to: "m2".into(),
                input: "x".into(),
                source: "flow.input.y".into(),
            },
        )
        .unwrap();
        assert_eq!(
            again[1].inputs,
            vec![("x".to_owned(), "flow.input.y".to_owned())]
        );
    }

    #[test]
    fn rename_updates_references_in_expressions() {
        let mut m2 = module("m2");
        m2.inputs.push(("x".into(), "m1.out".into()));
        let renamed = apply(
            &[module("m1"), m2],
            &Action::RenameModule {
                from: "m1".into(),
                to: "m5".into(),
            },
        )
        .unwrap();
        assert_eq!(renamed[0].id, "m5");
        assert_eq!(renamed[1].inputs[0].1, "m5.out");
    }

    #[test]
    fn apply_does_not_mutate_the_original_list() {
        let original = vec![module("m1")];
        let _ = apply(&original, &Action::RemoveModule("m1".into())).unwrap();
        assert_eq!(original.len(), 1);
    }
}
