use super::*;

#[test]
fn runstore_shutdown_drains_pending_in_order_then_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let log = RunLog::open(dir.path().join("run.log"));
    let mut draining = DrainingLog::new(log.clone());
    draining.enqueue("one").unwrap();
    draining.enqueue("two").unwrap();
    assert_eq!(draining.shutdown(), Ok(2));
    assert_eq!(draining.enqueue("late"), Err(DrainError::Closed));
    assert_eq!(log.recover().unwrap().records, vec!["one", "two"]);
}

#[test]
fn runstore_retry_preserves_order_and_stops_at_first_failure() {
    let mut queue = RetryQueue::new(3);
    queue.push("a");
    queue.push("b");
    queue.push("c");
    let mut written: Vec<String> = Vec::new();
    let mut fail_b_once = true;
    let first = queue.run(|payload| {
        if payload == "b" && fail_b_once {
            fail_b_once = false;
            return Err(());
        }
        written.push(payload.to_owned());
        Ok(())
    });
    assert_eq!(first, 1);
    assert_eq!(queue.len(), 2);
    let second = queue.run(|payload| {
        written.push(payload.to_owned());
        Ok::<(), ()>(())
    });
    assert_eq!(second, 2);
    assert_eq!(written, vec!["a", "b", "c"]);
    assert!(queue.is_empty());
}

#[test]
fn runstore_retry_dead_letters_after_max_attempts_and_keeps_going() {
    let mut queue = RetryQueue::new(2);
    queue.push("x");
    queue.push("y");
    assert_eq!(queue.run(|_| Err::<(), ()>(())), 0);
    let delivered = queue.run(|payload| if payload == "x" { Err(()) } else { Ok(()) });
    assert_eq!(delivered, 1);
    assert_eq!(queue.dead_letters(), &["x".to_owned()]);
    assert!(queue.is_empty());
}
