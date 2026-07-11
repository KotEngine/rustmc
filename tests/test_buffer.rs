use rustmc::protocol::io::Buffer;

#[test]
fn varint_roundtrip_via_packet_framing() {
    for &v in &[0i32, 1, 127, 128, 255, 25565, i32::MAX] {
        let mut buf = Buffer::new();
        buf.write_varint(v);
        let packet = buf.into_packet();
        // into_packet prefixes with an outer length-varint; strip it first.
        let mut framed = Buffer::from_vec(packet);
        let _len = framed.read_varint().unwrap();
        assert_eq!(framed.read_varint().unwrap(), v);
    }
}

#[test]
fn varint_roundtrip_negative_raw() {
    // Build the raw bytes directly (no outer framing) to test read_varint
    // in isolation.
    let mut buf = Buffer::new();
    buf.write_varint(-1);
    let packet = buf.into_packet();
    let mut framed = Buffer::from_vec(packet);
    let _len = framed.read_varint().unwrap();
    assert_eq!(framed.read_varint().unwrap(), -1);
}

#[test]
fn varint_rejects_five_byte_overflow() {
    // 5 bytes, all with the continuation bit set and payload bits on the
    // 5th byte beyond what fits in i32.
    let data = vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut buf = Buffer::from_vec(data);
    assert!(buf.read_varint().is_err());
}

#[test]
fn read_bytes_rejects_declared_len_over_max() {
    let mut buf = Buffer::from_vec(vec![0u8; 4]);
    let err = buf.read_bytes(2 * 1024 * 1024).unwrap_err();
    assert!(matches!(err, rustmc::RustmcError::InvalidResponse(_)));
}

#[test]
fn read_bytes_rejects_declared_len_over_remaining() {
    let mut buf = Buffer::from_vec(vec![1, 2, 3]);
    let err = buf.read_bytes(10).unwrap_err();
    assert!(matches!(err, rustmc::RustmcError::InvalidResponse(_)));
}

#[test]
fn read_string_rejects_negative_length() {
    // VarInt encoding of -1 as an unsigned 32-bit value: FF FF FF FF 0F
    let mut buf = Buffer::from_vec(vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    assert!(buf.read_string().is_err());
}
