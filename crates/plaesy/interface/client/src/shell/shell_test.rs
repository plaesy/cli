use super::*;

#[test]
fn breadcrumb_names_the_section_then_the_item() {
    assert_eq!(breadcrumb(&Page::FlowList), vec!["Flow".to_owned()]);
    assert_eq!(
        breadcrumb(&Page::FlowEditor("f1".to_owned())),
        vec!["Flow".to_owned(), "f1".to_owned()]
    );
    assert_eq!(
        breadcrumb(&Page::DataEditor("crm".to_owned())),
        vec!["Data".to_owned(), "crm".to_owned()]
    );
    assert_eq!(
        breadcrumb(&Page::NotFound),
        vec!["Tidak ditemukan".to_owned()]
    );
}

#[test]
fn shell_routes_known_paths() {
    assert_eq!(route("/flows"), Page::FlowList);
    assert_eq!(route("/flows/f1"), Page::FlowEditor("f1".to_owned()));
    assert_eq!(route("/data/crm"), Page::DataEditor("crm".to_owned()));
    assert_eq!(route("/"), Page::FlowList);
}

#[test]
fn shell_decodes_slash_in_document_kind() {
    assert_eq!(
        route("/documents/flow-ir%2F1/checkout"),
        Page::Document {
            kind: "flow-ir/1".to_owned(),
            id: "checkout".to_owned()
        }
    );
}

#[test]
fn shell_unknown_path_is_not_found() {
    assert_eq!(route("/tidak/ada/sama/sekali"), Page::NotFound);
    assert_eq!(route("/documents/x"), Page::NotFound);
}

#[test]
fn shell_guard_blocks_pages_without_permission() {
    let guard = Guard::new(&["flow:read"]);
    assert!(guard.allows(&Page::FlowList));
    assert!(!guard.allows(&Page::DataList));
    assert!(guard.allows(&Page::NotFound), "404 tidak memerlukan izin");
}

#[test]
fn shell_nav_lists_only_allowed_pages() {
    assert_eq!(nav(&Guard::new(&["data:read"])), vec![Page::DataList]);
    assert!(nav(&Guard::default()).is_empty());
}
