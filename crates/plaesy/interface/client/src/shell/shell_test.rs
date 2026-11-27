use super::*;

#[test]
fn ids_with_reserved_characters_round_trip() {
    for id in ["a/b", "x y", "100%", "ü-ß", "q?r#s", "%2F"] {
        let flow = Page::FlowEditor(id.to_owned());
        assert_eq!(route(&path_for(&flow)), flow, "flow {id}");
        let data = Page::DataEditor(id.to_owned());
        assert_eq!(route(&path_for(&data)), data, "data {id}");
        let doc = Page::Document {
            kind: "flow-ir/1".to_owned(),
            id: id.to_owned(),
        };
        assert_eq!(route(&path_for(&doc)), doc, "document {id}");
    }
}

#[test]
fn encoded_slash_in_an_id_stays_one_segment() {
    assert_eq!(route("/flows/a%2Fb"), Page::FlowEditor("a/b".to_owned()));
    assert_eq!(
        path_for(&Page::FlowEditor("a/b".to_owned())),
        "/flows/a%2Fb"
    );
}

#[test]
fn decode_keeps_malformed_escapes_as_they_are() {
    assert_eq!(decode_part("100%"), "100%");
    assert_eq!(decode_part("%zz"), "%zz");
    assert_eq!(decode_part("a%2fb"), "a/b");
}

#[test]
fn path_for_round_trips_through_route() {
    let pages = [
        Page::FlowList,
        Page::FlowEditor("flw_1".to_owned()),
        Page::DataList,
        Page::DataEditor("dat_1".to_owned()),
        Page::Document {
            kind: "flow-ir/1".to_owned(),
            id: "x".to_owned(),
        },
    ];
    for page in pages {
        assert_eq!(route(&path_for(&page)), page);
    }
    assert_eq!(path_for(&Page::FlowList), "/flows");
}

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
