use super::ledbat::LedbatController;
use super::packet::{PacketHeader, PacketType};
use std::time::{Duration, Instant};

#[test]
fn test_ledbat_cwnd_adjustments() {
    let mut controller = LedbatController::new();
    let now = Instant::now();

    // Default cwnd should be 3000
    assert_eq!(controller.cwnd(), 3000);

    // Add delay samples to establish base delay (e.g. 50ms = 50_000us)
    controller.add_delay_sample(50_000, now);

    // Receive an ACK with queuing delay lower than target (100ms = 100_000us)
    // E.g. delay is 70ms (70_000us). Queuing delay is 20ms (20_000us).
    // off_target = 80ms > 0, so cwnd should increase.
    controller.on_ack(70_000, 1000, now + Duration::from_millis(10));
    assert!(controller.cwnd() > 3000);

    // Receive an ACK with queuing delay higher than target
    // E.g. delay is 200ms (200_000us). Queuing delay is 150ms (150_000us).
    // off_target = -50ms < 0, so cwnd should decrease.
    let prev_cwnd = controller.cwnd();
    controller.on_ack(200_000, 1000, now + Duration::from_millis(20));
    assert!(controller.cwnd() < prev_cwnd);
}

#[test]
fn test_packet_header_serialization() {
    let header = PacketHeader {
        packet_type: PacketType::Syn,
        version: 1,
        extension: 0,
        connection_id: 1234,
        timestamp_us: 100_000,
        timestamp_difference_us: 5_000,
        wnd_size: 65536,
        seq_nr: 42,
        ack_nr: 41,
    };

    let mut buf = [0u8; PacketHeader::LEN];
    header.serialize(&mut buf);

    let deserialized = PacketHeader::deserialize(&buf).unwrap();

    assert_eq!(deserialized.packet_type, PacketType::Syn);
    assert_eq!(deserialized.version, 1);
    assert_eq!(deserialized.extension, 0);
    assert_eq!(deserialized.connection_id, 1234);
    assert_eq!(deserialized.timestamp_us, 100_000);
    assert_eq!(deserialized.timestamp_difference_us, 5_000);
    assert_eq!(deserialized.wnd_size, 65536);
    assert_eq!(deserialized.seq_nr, 42);
    assert_eq!(deserialized.ack_nr, 41);
}
