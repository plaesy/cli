use super::*;

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn pos(x: i64, y: i64) -> Position {
    Position { x, y }
}

#[test]
fn chain_places_each_module_in_its_own_column() {
    let out = layout(&ids(&["a", "b", "c"]), &edges(&[("a", "b"), ("b", "c")])).unwrap();
    assert_eq!(out["a"].x, 0);
    assert_eq!(out["b"].x, COLUMN_GAP);
    assert_eq!(out["c"].x, 2 * COLUMN_GAP);
}

#[test]
fn column_is_longest_path_from_source() {
    // c punya dua jalur: a -> c (panjang 1) dan a -> b -> c (panjang 2). Harus kolom 2.
    let out = layout(
        &ids(&["a", "b", "c"]),
        &edges(&[("a", "c"), ("a", "b"), ("b", "c")]),
    )
    .unwrap();
    assert_eq!(out["a"].x, 0);
    assert_eq!(out["b"].x, COLUMN_GAP);
    assert_eq!(out["c"].x, 2 * COLUMN_GAP);
}

#[test]
fn diamond_puts_siblings_in_same_column() {
    let out = layout(
        &ids(&["a", "b", "c", "d"]),
        &edges(&[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]),
    )
    .unwrap();
    assert_eq!(out["b"].x, COLUMN_GAP);
    assert_eq!(out["c"].x, COLUMN_GAP);
    assert_eq!(out["d"].x, 2 * COLUMN_GAP);
    assert_ne!(out["b"].y, out["c"].y);
}

#[test]
fn barycenter_removes_crossing_between_two_columns() {
    // Urutan awal kolom 1 adalah [c, d], tetapi a -> d dan b -> c menyilang.
    // Sapuan barycenter harus menaruh d di atas c.
    let out = layout(&ids(&["a", "b", "c", "d"]), &edges(&[("a", "d"), ("b", "c")])).unwrap();
    assert!(out["d"].y < out["c"].y);
    assert_eq!(out["a"].y, out["d"].y);
    assert_eq!(out["b"].y, out["c"].y);
}

#[test]
fn isolated_modules_share_first_column() {
    let out = layout(&ids(&["x", "y"]), &[]).unwrap();
    assert_eq!(out["x"].x, 0);
    assert_eq!(out["y"].x, 0);
    assert_ne!(out["x"].y, out["y"].y);
}

#[test]
fn empty_input_yields_empty_layout() {
    let out = layout(&[], &[]).unwrap();
    assert!(out.is_empty());
}

#[test]
fn all_positions_are_non_negative() {
    let out = layout(&ids(&["a", "b", "c", "d"]), &edges(&[("c", "a"), ("d", "b")])).unwrap();
    for p in out.values() {
        assert!(p.x >= 0 && p.y >= 0, "posisi negatif: {p:?}");
    }
}

#[test]
fn two_node_cycle_is_rejected() {
    let err = layout(&ids(&["a", "b"]), &edges(&[("a", "b"), ("b", "a")])).unwrap_err();
    match err {
        LayoutError::Cycle(mut ids) => {
            ids.sort();
            assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
        }
        other => panic!("diharapkan Cycle, dapat {other:?}"),
    }
}

#[test]
fn self_loop_is_a_cycle() {
    let err = layout(&ids(&["a"]), &edges(&[("a", "a")])).unwrap_err();
    assert_eq!(err, LayoutError::Cycle(ids(&["a"])));
}

#[test]
fn cycle_reports_members_and_downstream_not_upstream() {
    // a masuk ke siklus b <-> c. a itu sumber dan bukan bagian siklus.
    let err = layout(
        &ids(&["a", "b", "c"]),
        &edges(&[("a", "b"), ("b", "c"), ("c", "b")]),
    )
    .unwrap_err();
    match err {
        LayoutError::Cycle(ids) => {
            assert!(ids.contains(&"b".to_string()));
            assert!(ids.contains(&"c".to_string()));
            assert!(!ids.contains(&"a".to_string()));
        }
        other => panic!("diharapkan Cycle, dapat {other:?}"),
    }
}

#[test]
fn unknown_edge_endpoint_is_an_error() {
    let err = layout(&ids(&["a"]), &edges(&[("a", "zzz")])).unwrap_err();
    assert_eq!(err, LayoutError::UnknownModule("zzz".to_string()));
}

#[test]
fn duplicate_module_is_an_error() {
    let err = layout(&ids(&["a", "a"]), &[]).unwrap_err();
    assert_eq!(err, LayoutError::DuplicateModule("a".to_string()));
}

#[test]
fn move_module_updates_position() {
    let mut map = layout(&ids(&["a", "b"]), &edges(&[("a", "b")])).unwrap();
    move_module(&mut map, "a", pos(500, 40)).unwrap();
    assert_eq!(map["a"], pos(500, 40));
    assert_eq!(map["b"].x, COLUMN_GAP);
}

#[test]
fn move_module_rejects_negative_coordinates_and_keeps_old_position() {
    let mut map = layout(&ids(&["a"]), &[]).unwrap();
    let before = map["a"];

    let err = move_module(&mut map, "a", pos(-1, 0)).unwrap_err();
    assert_eq!(err, LayoutError::NegativePosition { x: -1, y: 0 });

    let err = move_module(&mut map, "a", pos(0, -5)).unwrap_err();
    assert_eq!(err, LayoutError::NegativePosition { x: 0, y: -5 });

    assert_eq!(map["a"], before);
}

#[test]
fn move_module_rejects_unknown_module() {
    let mut map = layout(&ids(&["a"]), &[]).unwrap();
    let err = move_module(&mut map, "nope", pos(0, 0)).unwrap_err();
    assert_eq!(err, LayoutError::MissingModule("nope".to_string()));
}
