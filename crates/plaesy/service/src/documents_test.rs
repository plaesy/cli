use crate::documents::{list, load, remove, save, DocumentError};
use crate::port::{DocumentStore, StoreError};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Penyimpanan di memori untuk tes. Tidak bergantung pada adapter.
#[derive(Default)]
struct MemoryStore {
    docs: RefCell<BTreeMap<(String, String), Value>>,
}

impl DocumentStore for MemoryStore {
    fn put(&self, kind: &str, id: &str, document: &Value) -> Result<(), StoreError> {
        self.docs
            .borrow_mut()
            .insert((kind.to_owned(), id.to_owned()), document.clone());
        Ok(())
    }

    fn get(&self, kind: &str, id: &str) -> Result<Option<Value>, StoreError> {
        Ok(self
            .docs
            .borrow()
            .get(&(kind.to_owned(), id.to_owned()))
            .cloned())
    }

    fn list(&self, kind: &str) -> Result<Vec<String>, StoreError> {
        Ok(self
            .docs
            .borrow()
            .keys()
            .filter(|(k, _)| k == kind)
            .map(|(_, id)| id.clone())
            .collect())
    }

    fn delete(&self, kind: &str, id: &str) -> Result<bool, StoreError> {
        Ok(self
            .docs
            .borrow_mut()
            .remove(&(kind.to_owned(), id.to_owned()))
            .is_some())
    }
}

/// Dokumen contoh pertama dari setiap folder contoh IR (ADR).
fn first_example(dir: &str) -> Value {
    let folder: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/adr/examples")
        .join(dir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&folder)
        .unwrap_or_else(|e| panic!("{}: {e}", folder.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let text = std::fs::read_to_string(&files[0]).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn documents_every_ir_example_saves_and_loads_back_identical() {
    let store = MemoryStore::default();
    for (dir, ir) in [
        ("flow-ir-1", "flow-ir/1"),
        ("data-ir-1", "data-ir/1"),
        ("api-ir-1", "api-ir/1"),
        ("app-ir-1", "app-ir/1"),
    ] {
        let doc = first_example(dir);
        let id = save(&store, &doc).unwrap_or_else(|e| panic!("{dir}: {e}"));
        assert_eq!(load(&store, ir, &id).unwrap(), Some(doc), "{dir}");
    }
}

#[test]
fn documents_invalid_document_is_refused_before_storing() {
    let store = MemoryStore::default();
    let mut doc = first_example("data-ir-1");
    doc["tables"][0]["primary_key"] = json!(["tidak_ada"]);
    let error = save(&store, &doc).unwrap_err();
    assert!(
        matches!(error, DocumentError::Invalid { ref ir, .. } if ir.as_deref() == Some("data-ir/1"))
    );
    assert!(
        store.list("data-ir/1").unwrap().is_empty(),
        "tidak boleh tersimpan"
    );
}

#[test]
fn documents_missing_id_is_refused() {
    let store = MemoryStore::default();
    let mut doc = first_example("data-ir-1");
    doc.as_object_mut().unwrap().remove("id");
    assert_eq!(save(&store, &doc).unwrap_err(), DocumentError::MissingId);
}

#[test]
fn documents_corrupt_stored_document_is_not_returned_silently() {
    let store = MemoryStore::default();
    let mut doc = first_example("api-ir-1");
    let id = save(&store, &doc).unwrap();
    // Endpoint duplikat pasti melanggar aturan semantik api-ir.
    let first = doc["endpoints"][0].clone();
    doc["endpoints"].as_array_mut().unwrap().push(first);
    store.put("api-ir/1", &id, &doc).unwrap();
    assert!(matches!(
        load(&store, "api-ir/1", &id),
        Err(DocumentError::Invalid { .. })
    ));
}

#[test]
fn documents_list_and_remove_are_scoped_to_the_ir() {
    let store = MemoryStore::default();
    let id = save(&store, &first_example("data-ir-1")).unwrap();
    assert_eq!(list(&store, "data-ir/1").unwrap(), vec![id.clone()]);
    assert!(list(&store, "api-ir/1").unwrap().is_empty());
    assert!(remove(&store, "data-ir/1", &id).unwrap());
    assert_eq!(load(&store, "data-ir/1", &id).unwrap(), None);
}
