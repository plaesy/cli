use super::*;

#[test]
fn notifications_push_assigns_increasing_ids() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu").unwrap();
    let b = queue.push(Level::Error, "dua").unwrap();
    assert!(b > a);
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.list()[1].level, Level::Error);
}

#[test]
fn notifications_empty_message_is_refused() {
    let mut queue = Queue::new(10);
    assert_eq!(
        queue.push(Level::Warning, "   "),
        Err(NotificationError::EmptyMessage)
    );
    assert!(queue.is_empty());
}

#[test]
fn notifications_dismiss_removes_only_that_item() {
    let mut queue = Queue::new(10);
    let a = queue.push(Level::Info, "satu").unwrap();
    let _b = queue.push(Level::Info, "dua").unwrap();
    assert!(queue.dismiss(a));
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.list()[0].message, "dua");
    assert!(!queue.dismiss(a), "menutup dua kali tidak berefek");
}

#[test]
fn notifications_capacity_drops_the_oldest() {
    let mut queue = Queue::new(2);
    queue.push(Level::Info, "satu").unwrap();
    queue.push(Level::Info, "dua").unwrap();
    queue.push(Level::Info, "tiga").unwrap();
    let messages: Vec<&str> = queue.list().iter().map(|n| n.message.as_str()).collect();
    assert_eq!(messages, ["dua", "tiga"]);
}
