//! Unit tests for `rga.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn replica(id: &str) -> Replica {
    Replica::new(ReplicaId::new(id).expect("id tes valid"))
}

#[test]
fn empty_replica_id_is_rejected() {
    assert_eq!(ReplicaId::new(""), Err(CollabError::EmptyReplicaId));
}

#[test]
fn out_of_range_index_is_an_error() {
    let mut a = replica("a");
    assert_eq!(
        a.insert_at(1, 'x'),
        Err(CollabError::IndexOutOfRange { index: 1, len: 0 })
    );
    assert_eq!(
        a.delete_at(0),
        Err(CollabError::IndexOutOfRange { index: 0, len: 0 })
    );
}

#[test]
fn local_edits_build_text() {
    let mut a = replica("a");
    a.insert_at(0, 'h').expect("sisip");
    a.insert_at(1, 'i').expect("sisip");
    a.insert_at(0, '>').expect("sisip");
    assert_eq!(a.text(), ">hi");
    a.delete_at(1).expect("hapus");
    assert_eq!(a.text(), ">i");
    assert_eq!(a.len(), 2);
}

#[test]
fn two_replicas_converge_under_different_delivery_orders() {
    let mut a = replica("a");
    let mut b = replica("b");

    // Keadaan awal yang sama di kedua replika.
    let base = vec![
        a.insert_at(0, 'h').expect("sisip"),
        a.insert_at(1, 'i').expect("sisip"),
    ];
    for op in base {
        b.apply(op).expect("terapkan");
    }
    assert_eq!(a.text(), "hi");
    assert_eq!(b.text(), "hi");

    // Edit konkuren.
    let op_ax = a.insert_at(0, 'X').expect("sisip");
    let op_ad = a.delete_at(1).expect("hapus");
    let op_by = b.insert_at(2, 'Y').expect("sisip");
    let op_bz = b.insert_at(0, 'Z').expect("sisip");

    // Replika A menerima operasi B dalam urutan terbalik, B menerima operasi A
    // dalam urutan lain.
    a.apply(op_bz.clone()).expect("terapkan");
    a.apply(op_by.clone()).expect("terapkan");
    b.apply(op_ad.clone()).expect("terapkan");
    b.apply(op_ax.clone()).expect("terapkan");

    assert_eq!(a.text(), b.text());
    assert_eq!(a.text(), "ZXiY");
}

#[test]
fn concurrent_delete_of_same_char_converges() {
    let mut a = replica("a");
    let mut b = replica("b");
    let ins = a.insert_at(0, 'q').expect("sisip");
    b.apply(ins).expect("terapkan");

    let del_a = a.delete_at(0).expect("hapus");
    let del_b = b.delete_at(0).expect("hapus");

    a.apply(del_b).expect("terapkan");
    b.apply(del_a).expect("terapkan");

    assert_eq!(a.text(), "");
    assert_eq!(b.text(), "");
    assert_eq!(a.len(), 0);
    assert_eq!(b.len(), 0);
}

#[test]
fn duplicate_operations_are_ignored() {
    let mut a = replica("a");
    let mut b = replica("b");

    let ins = a.insert_at(0, 'k').expect("sisip");
    b.apply(ins.clone()).expect("terapkan");
    b.apply(ins.clone()).expect("terapkan ulang");
    assert_eq!(b.text(), "k");
    assert_eq!(b.history().len(), 1);

    let del = a.delete_at(0).expect("hapus");
    b.apply(del.clone()).expect("terapkan");
    b.apply(del.clone()).expect("terapkan ulang");
    assert_eq!(b.text(), "");
    assert_eq!(b.history().len(), 2);
}

#[test]
fn operations_arriving_before_their_dependencies_are_buffered() {
    let mut a = replica("a");
    let mut b = replica("b");

    let op1 = a.insert_at(0, 'a').expect("sisip");
    let op2 = a.insert_at(1, 'b').expect("sisip");

    b.apply(op2.clone()).expect("terapkan");
    assert_eq!(b.text(), "");
    assert_eq!(b.pending_len(), 1);

    // Pengiriman ulang op2 saat masih tertunda tidak menambah antrean.
    b.apply(op2).expect("terapkan");
    assert_eq!(b.pending_len(), 1);

    b.apply(op1).expect("terapkan");
    assert_eq!(b.text(), "ab");
    assert_eq!(b.pending_len(), 0);
}

#[test]
fn merge_from_converges_both_ways() {
    let mut a = replica("a");
    let mut b = replica("b");
    a.insert_at(0, 'a').expect("sisip");
    b.insert_at(0, 'b').expect("sisip");
    a.insert_at(1, '1').expect("sisip");
    b.insert_at(1, '2').expect("sisip");

    let snapshot_a = a.text();
    let snapshot_b = b.text();
    a.merge_from(&b).expect("gabung");
    b.merge_from(&a).expect("gabung");

    assert_eq!(a.text(), b.text());
    assert_eq!(a.len(), 4);
    assert_ne!(snapshot_a, a.text());
    assert_ne!(snapshot_b, b.text());
}

#[test]
fn remote_operation_advances_lamport_clock() {
    let mut a = replica("a");
    let mut b = replica("b");
    for i in 0..5 {
        a.insert_at(i, 'x').expect("sisip");
    }
    for op in a.history().to_vec() {
        b.apply(op).expect("terapkan");
    }
    assert!(b.clock() >= 5);
    let next = b.insert_at(0, 'y').expect("sisip");
    match next {
        Op::Insert { id, .. } => assert!(id.lamport > 5),
        Op::Delete { .. } => panic!("sisip diharapkan"),
    }
}

#[test]
fn malformed_operations_are_rejected() {
    let mut a = replica("a");
    let id = OpId {
        lamport: 1,
        replica: ReplicaId::new("a").expect("id tes valid"),
    };
    let self_parent = Op::Insert {
        id: id.clone(),
        after: Some(id.clone()),
        ch: 'z',
    };
    assert!(matches!(
        a.apply(self_parent),
        Err(CollabError::InvalidOp(_))
    ));

    let zero_clock = Op::Delete {
        target: id,
        lamport: 0,
    };
    assert!(matches!(
        a.apply(zero_clock),
        Err(CollabError::InvalidOp(_))
    ));
}
