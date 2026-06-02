use super::*;

#[test]
fn test_bitfield_creation() {
    let bf = Bitfield::new(10);
    assert_eq!(bf.len(), 10);
    assert_eq!(bf.count_set(), 0);
    assert!(!bf.get(0));
}

#[test]
fn test_bitfield_set_get() {
    let mut bf = Bitfield::new(10);
    bf.set(5, true);
    assert!(bf.get(5));
    assert_eq!(bf.count_set(), 1);
    bf.set(5, false);
    assert!(!bf.get(5));
    assert_eq!(bf.count_set(), 0);
}

#[test]
fn test_bitfield_is_complete() {
    let mut bf = Bitfield::new(3);
    assert!(!bf.is_complete());
    bf.set(0, true);
    bf.set(1, true);
    bf.set(2, true);
    assert!(bf.is_complete());
}

#[test]
fn test_bitfield_serialization() {
    let mut bf = Bitfield::new(10);
    bf.set(0, true);
    bf.set(7, true);
    bf.set(8, true);

    let bytes = bf.as_bytes();
    // 10 pieces = 2 bytes (8 + 2 bits)
    assert_eq!(bytes.len(), 2);
    // Byte 0: 10000001 (129)
    assert_eq!(bytes[0], 0b10000001);
    // Byte 1: 10000000 (128)
    assert_eq!(bytes[1], 0b10000000);

    let bf2 = Bitfield::from_bytes(&bytes, 10);
    assert!(bf2.get(0));
    assert!(bf2.get(7));
    assert!(bf2.get(8));
    assert!(!bf2.get(1));
}
