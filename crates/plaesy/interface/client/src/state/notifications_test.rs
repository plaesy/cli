use super::*;

#[test]
fn notifications_push_assigns_increasing_ids() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    let b = queue.push(Level::Error, "dua", 0).unwrap();
    assert!(b > a);
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.list()[1].level, Level::Error);
}

#[test]
fn notifications_levels_are_info_warning_and_error() {
    let mut queue = Queue::new(10);
    queue.push(Level::Info, "a", 0).unwrap();
    queue.push(Level::Warning, "b", 0).unwrap();
    queue.push(Level::Error, "c", 0).unwrap();
    let levels: Vec<Level> = queue.list().iter().map(|n| n.level).collect();
    assert_eq!(levels, [Level::Info, Level::Warning, Level::Error]);
}

#[test]
fn notifications_empty_message_is_refused() {
    let mut queue = Queue::new(10);
    assert_eq!(
        queue.push(Level::Warning, "   ", 0),
        Err(NotificationError::EmptyMessage)
    );
    assert!(queue.is_empty());
}

#[test]
fn notifications_dismiss_removes_only_that_item() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    let _b = queue.push(Level::Info, "dua", 0).unwrap();
    assert!(queue.dismiss(a));
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.list()[0].message, "dua");
    assert!(!queue.dismiss(a), "menutup dua kali tidak berefek");
}

#[test]
fn notifications_ids_are_not_reused_after_dismiss() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    assert!(queue.dismiss(a));
    let b = queue.push(Level::Info, "dua", 0).unwrap();
    assert!(b > a);
}

#[test]
fn notifications_capacity_drops_the_oldest() {
    let mut queue = Queue::new(2);
    queue.push(Level::Info, "satu", 0).unwrap();
    queue.push(Level::Info, "dua", 0).unwrap();
    queue.push(Level::Info, "tiga", 0).unwrap();
    let messages: Vec<&str> = queue.list().iter().map(|n| n.message.as_str()).collect();
    assert_eq!(messages, ["dua", "tiga"]);
}

#[test]
fn notifications_capacity_zero_is_raised_to_one() {
    let mut queue = Queue::new(0);
    assert_eq!(queue.capacity(), 1);
    queue.push(Level::Info, "satu", 0).unwrap();
    queue.push(Level::Info, "dua", 0).unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.list()[0].message, "dua");
}

#[test]
fn notifications_new_items_start_unread() {
    let mut queue = Queue::new(10);
    queue.push(Level::Info, "satu", 0).unwrap();
    queue.push(Level::Info, "dua", 0).unwrap();
    assert_eq!(queue.unread_count(), 2);
    assert!(!queue.list()[0].read);
}

#[test]
fn notifications_mark_read_changes_only_that_item() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    queue.push(Level::Info, "dua", 0).unwrap();
    assert!(queue.mark_read(a));
    assert!(queue.list()[0].read);
    assert!(!queue.list()[1].read);
    assert_eq!(queue.unread_count(), 1);
}

#[test]
fn notifications_mark_read_unknown_id_returns_false() {
    let mut queue = Queue::new(10);
    queue.push(Level::Info, "satu", 0).unwrap();
    assert!(!queue.mark_read(999));
    assert_eq!(queue.unread_count(), 1);
}

#[test]
fn notifications_mark_read_is_idempotent() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    assert!(queue.mark_read(a));
    assert!(queue.mark_read(a));
    assert_eq!(queue.unread_count(), 0);
}

#[test]
fn notifications_mark_all_read_clears_unread() {
    let mut queue = Queue::new(10);
    queue.push(Level::Info, "satu", 0).unwrap();
    queue.push(Level::Warning, "dua", 0).unwrap();
    queue.push(Level::Error, "tiga", 0).unwrap();
    queue.mark_all_read();
    assert_eq!(queue.unread_count(), 0);
    assert!(queue.list().iter().all(|n| n.read));
}

#[test]
fn notifications_overflow_drops_oldest_even_if_unread() {
    let mut queue = Queue::new(2);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    let b = queue.push(Level::Info, "dua", 0).unwrap();
    queue.mark_read(b);
    let c = queue.push(Level::Info, "tiga", 0).unwrap();
    let ids: Vec<u64> = queue.list().iter().map(|n| n.id).collect();
    assert_eq!(ids, [b, c]);
    assert!(!ids.contains(&a));
    assert_eq!(queue.unread_count(), 1);
}

#[test]
fn notifications_prune_removes_items_older_than_max_age() {
    let mut queue = Queue::new(10);
    queue.push(Level::Info, "lama", 1_000).unwrap();
    queue.push(Level::Info, "batas", 5_000).unwrap();
    queue.push(Level::Info, "baru", 9_000).unwrap();
    // now = 10_000, max_age = 5_000: "lama" berumur 9_000 dibuang; "batas" berumur tepat 5_000 tetap.
    let removed = queue.prune_older_than(10_000, 5_000);
    assert_eq!(removed, 1);
    let messages: Vec<&str> = queue.list().iter().map(|n| n.message.as_str()).collect();
    assert_eq!(messages, ["batas", "baru"]);
}

#[test]
fn notifications_prune_removes_read_and_unread_alike() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu", 0).unwrap();
    queue.push(Level::Info, "dua", 0).unwrap();
    queue.mark_read(a);
    assert_eq!(queue.prune_older_than(100, 10), 2);
    assert!(queue.is_empty());
}

#[test]
fn notifications_prune_keeps_items_when_clock_went_backwards() {
    let mut queue = Queue::new(10);
    queue.push(Level::Info, "satu", 9_000).unwrap();
    assert_eq!(queue.prune_older_than(1_000, 0), 0);
    assert_eq!(queue.len(), 1);
}

#[test]
fn notifications_prune_on_empty_queue_removes_nothing() {
    let mut queue = Queue::new(10);
    assert_eq!(queue.prune_older_than(10_000, 0), 0);
    assert!(queue.is_empty());
}
