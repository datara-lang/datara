//! v1.4.4: Universal Systems, Networking, Binary Layout & White-Hat Primitives Tests.

use forgen::driver::ForgenCompiler;

fn run_datara(source: &str, name: &str) -> (String, i32) {
    let compiler = ForgenCompiler::new("release");
    let res = compiler.compile_source_native(source, name, None);
    assert!(
        res.success,
        "compilation failed for {}: {:?}\n{}",
        name, res.error, res.diagnostics
    );

    let exe = res.exe_path.clone().expect("must produce a native .exe");
    let (stdout, stderr, code, _) = compiler
        .cranelift
        .run_executable(&exe, &[])
        .expect("must run native exe");

    let _ = std::fs::remove_file(&exe);
    let _ = std::fs::remove_file(exe.with_extension("obj"));
    if code != 0 {
        eprintln!("STDERR: {}", stderr);
    }
    (stdout.trim().replace("\r\n", "\n"), code)
}

#[test]
fn test_endian_byte_swapping() {
    let src = r#"
fn main() -> Int {
    let original = 0x12345678
    let swapped = bswap32(original)
    let restored = bswap32(swapped)
    if restored == original {
        print("bswap32 ok")
        return 0
    }
    return 1
}
"#;
    let (stdout, code) = run_datara(src, "test_bswap.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "bswap32 ok");
}

#[test]
fn test_slice_view_read_write() {
    let src = r#"
fn main() -> Int {
    let s = slice_alloc(32)
    s.set_byte(0, 65)
    s.write_u16_be(2, 4000)
    s.write_u32_be(4, 987654321)

    let b0 = s.get_byte(0)
    let u16_val = s.read_u16_be(2)
    let u32_val = s.read_u32_be(4)

    if b0 == 65 && u16_val == 4000 && u32_val == 987654321 {
        print("slice rw ok")
        s.free()
        return 0
    }
    s.free()
    return 1
}
"#;
    let (stdout, code) = run_datara(src, "test_slice_rw.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "slice rw ok");
}

#[test]
fn test_zero_copy_subslice() {
    let src = r#"
fn main() -> Int {
    let s = slice_alloc(64)
    s.write_u32_be(16, 0xCAFEBABE)

    let sub = s.subslice(16, 16)
    let magic = sub.read_u32_be(0)

    if magic == 0xCAFEBABE && sub.len() == 16 {
        print("subslice ok")
        s.free()
        return 0
    }
    s.free()
    return 1
}
"#;
    let (stdout, code) = run_datara(src, "test_subslice.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "subslice ok");
}

#[test]
fn test_binary_protocol_packet_parsing() {
    let src = r#"
fn main() -> Int {
    // Simulate a 20-byte IP/binary wire header
    let pkt = slice_alloc(20)
    // byte 0: Version (4) + IHL (5) -> 0x45
    pkt.set_byte(0, 0x45)
    // bytes 2-3: Total length = 1500
    pkt.write_u16_be(2, 1500)
    // bytes 4-5: Identification = 0xABCD
    pkt.write_u16_be(4, 0xABCD)
    // byte 8: TTL = 64
    pkt.set_byte(8, 64)
    // byte 9: Protocol = 6 (TCP)
    pkt.set_byte(9, 6)

    let v_ihl = pkt.get_byte(0)
    let total_len = pkt.read_u16_be(2)
    let ident = pkt.read_u16_be(4)
    let ttl = pkt.get_byte(8)
    let proto = pkt.get_byte(9)

    if v_ihl == 0x45 && total_len == 1500 && ident == 0xABCD && ttl == 64 && proto == 6 {
        print("packet parse ok")
        pkt.free()
        return 0
    }
    pkt.free()
    return 1
}
"#;
    let (stdout, code) = run_datara(src, "test_packet.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "packet parse ok");
}

#[test]
fn test_packed_struct_layout_and_isolation() {
    let src = r#"
@packed
class WireFrame {
    magic: Byte
    flag: Bool
    id: Int
}

fn main() -> Int {
    let frame = WireFrame { magic: 0x42, flag: true, id: 0x0102030405060708 }
    if frame.magic == 0x42 && frame.flag && frame.id == 0x0102030405060708 {
        print("packed struct ok")
        return 0
    }
    return 1
}
"#;
    let (stdout, code) = run_datara(src, "test_packed_frame.dtr");
    assert_eq!(code, 0);
    assert_eq!(stdout, "packed struct ok");
}

