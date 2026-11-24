use super::*;

#[test]
fn bytes_within_capacity_are_kept_in_order() {
    let mut buf = OutputBuffer::new(8);
    buf.push(b"abc");
    buf.push(b"de");
    assert_eq!(buf.buffered_len(), 5);

    let chunk = buf.take();
    assert_eq!(chunk.data, b"abcde");
    assert_eq!(chunk.dropped_bytes, 0);
    assert!(!chunk.closed);
}

#[test]
fn oldest_bytes_are_dropped_and_counted_on_overflow() {
    let mut buf = OutputBuffer::new(4);
    buf.push(b"abc");
    buf.push(b"de");
    let chunk = buf.take();
    assert_eq!(chunk.data, b"bcde");
    assert_eq!(chunk.dropped_bytes, 1);
}

#[test]
fn single_write_larger_than_capacity_keeps_only_its_tail() {
    let mut buf = OutputBuffer::new(4);
    buf.push(b"abc");
    buf.push(b"de");
    buf.push(b"123456");

    let chunk = buf.take();
    assert_eq!(chunk.data, b"3456");
    // 1 byte dibuang saat "de" masuk, lalu 4 byte lama dan 2 byte input dibuang.
    assert_eq!(chunk.dropped_bytes, 1 + 4 + 2);
}

#[test]
fn take_resets_dropped_counter_and_empties_buffer() {
    let mut buf = OutputBuffer::new(2);
    buf.push(b"xyz");
    assert_eq!(buf.take().dropped_bytes, 1);

    let second = buf.take();
    assert!(second.data.is_empty());
    assert_eq!(second.dropped_bytes, 0);
}

#[test]
fn closed_flag_is_reported_by_take() {
    let mut buf = OutputBuffer::new(4);
    buf.mark_closed();
    assert!(buf.take().closed);
}
