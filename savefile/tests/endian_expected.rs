//! Expected-bytes tests for the on-disk format.
//!
//! The savefile disk format is little endian on every host. These tests assert the
//! exact bytes produced, so that they catch any place where the in-memory (native)
//! representation leaks into the output. They are meant to be run both natively and
//! on a big-endian target under miri, e.g.:
//!
//! `cargo +nightly miri test -p savefile --test endian_expected --target s390x-unknown-linux-gnu`

use savefile::prelude::*;
use savefile_derive::Savefile;

#[derive(Savefile, Debug, PartialEq, Clone, Copy)]
#[repr(C)]
struct PackedStruct {
    a: u32,
    b: u16,
    c: u16,
    d: u64,
}

#[derive(Savefile, Debug, PartialEq, Clone)]
struct MixedStruct {
    x: u8,
    y: u16,
    z: u32,
    s: String,
    v: Vec<u32>,
    arr: [u16; 3],
}

#[derive(Savefile, Debug, PartialEq, Clone, Copy)]
#[repr(u16)]
enum Enum16 {
    A(u16, u16),
    B(u32),
    C,
}

#[derive(Savefile, Debug, PartialEq, Clone, Copy)]
#[repr(u32)]
enum Enum32 {
    A(u32),
    B(u64),
    C,
}

/// Serializes `value` without schema, checks that it roundtrips, and returns
/// the serialized bytes with the file header stripped.
fn roundtrip<T: WithSchema + Serialize + Deserialize + PartialEq + std::fmt::Debug>(value: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    save_noschema(&mut buf, 0, value).unwrap();
    let back: T = load_noschema(&mut &buf[..], 0).unwrap();
    assert_eq!(&back, value);
    assert_eq!(&buf[..8], b"savefile");
    buf.drain(..16);
    buf
}

#[test]
fn expected_packed_struct() {
    let bytes = roundtrip(&PackedStruct {
        a: 0x01020304,
        b: 0x0506,
        c: 0x0708,
        d: 0x090a0b0c0d0e0f10,
    });
    assert_eq!(
        bytes,
        [4, 3, 2, 1, 6, 5, 8, 7, 0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 9]
    );
}

#[test]
fn expected_vec_of_packed_struct() {
    let bytes = roundtrip(&vec![
        PackedStruct {
            a: 0x01020304,
            b: 0x0506,
            c: 0x0708,
            d: 0x090a0b0c0d0e0f10
        };
        2
    ]);
    let mut expected = Vec::new();
    expected.extend(2u64.to_le_bytes());
    for _ in 0..2 {
        expected.extend([4, 3, 2, 1, 6, 5, 8, 7, 0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 9]);
    }
    assert_eq!(bytes, expected);
}

#[test]
fn expected_primitive_vecs_and_arrays() {
    let bytes = roundtrip(&vec![0x0102u16, 0x0304]);
    assert_eq!(bytes, [2, 0, 0, 0, 0, 0, 0, 0, 2, 1, 4, 3]);
    let bytes = roundtrip(&[0x01020304u32, 0x05060708]);
    assert_eq!(bytes, [4, 3, 2, 1, 8, 7, 6, 5]);
    let bytes = roundtrip(&(0x0102u16, 0x03040506u32, 0x0708u16));
    assert_eq!(bytes, [2, 1, 6, 5, 4, 3, 8, 7]);
}

#[test]
fn expected_mixed_struct() {
    let bytes = roundtrip(&MixedStruct {
        x: 1,
        y: 0x0203,
        z: 0x04050607,
        s: "hi".into(),
        v: vec![0x0a0b0c0d],
        arr: [0x1112, 0x1314, 0x1516],
    });
    let mut expected = vec![1u8, 3, 2, 7, 6, 5, 4];
    expected.extend(2u64.to_le_bytes());
    expected.extend(b"hi");
    expected.extend(1u64.to_le_bytes());
    expected.extend([0x0d, 0x0c, 0x0b, 0x0a]);
    expected.extend([0x12, 0x11, 0x14, 0x13, 0x16, 0x15]);
    assert_eq!(bytes, expected);
}

#[test]
fn expected_enums() {
    assert_eq!(roundtrip(&Enum16::A(0x0102, 0x0304)), [0, 0, 2, 1, 4, 3]);
    assert_eq!(roundtrip(&Enum16::B(0x01020304)), [1, 0, 4, 3, 2, 1]);
    assert_eq!(roundtrip(&Enum16::C), [2, 0]);
    assert_eq!(roundtrip(&Enum32::A(0x01020304)), [0, 0, 0, 0, 4, 3, 2, 1]);
    assert_eq!(
        roundtrip(&Enum32::B(0x0102030405060708)),
        [1, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1]
    );
    assert_eq!(roundtrip(&Enum32::C), [2, 0, 0, 0]);
}

#[test]
fn enums_roundtrip_with_schema() {
    // Loading with schema exercises the derived enum variant field-offset probing.
    let value = vec![Enum16::A(1, 2), Enum16::B(3), Enum16::C];
    let mut buf = Vec::new();
    save(&mut buf, 0, &value).unwrap();
    let back: Vec<Enum16> = load(&mut &buf[..], 0).unwrap();
    assert_eq!(back, value);

    let value = vec![Enum32::A(1), Enum32::B(2), Enum32::C];
    let mut buf = Vec::new();
    save(&mut buf, 0, &value).unwrap();
    let back: Vec<Enum32> = load(&mut &buf[..], 0).unwrap();
    assert_eq!(back, value);
}

/// Expected serialization of a 40-bit bitvec with bits 0, 9 and 33 set.
#[cfg(any(feature = "bit-vec", feature = "bit-vec08"))]
fn expected_bitvec_bytes() -> Vec<u8> {
    let mut expected = Vec::new();
    expected.extend(40u64.to_le_bytes());
    expected.extend((8u64 | 1 << 63).to_le_bytes());
    expected.extend((1u32 | 1 << 9).to_le_bytes());
    expected.extend((1u32 << 1).to_le_bytes());
    expected
}

#[cfg(feature = "bit-vec")]
#[test]
fn expected_bitvec06() {
    let mut bv = bit_vec::BitVec::from_elem(40, false);
    bv.set(0, true);
    bv.set(9, true);
    bv.set(33, true);
    assert_eq!(roundtrip(&bv), expected_bitvec_bytes());
}

#[cfg(feature = "bit-vec08")]
#[test]
fn expected_bitvec08() {
    let mut bv = bit_vec08::BitVec::from_elem(40, false);
    bv.set(0, true);
    bv.set(9, true);
    bv.set(33, true);
    assert_eq!(roundtrip(&bv), expected_bitvec_bytes());
}

#[derive(Savefile, Debug, PartialEq, Clone, Copy)]
#[repr(C)]
struct ByteStruct {
    a: u8,
    b: [u8; 3],
    c: i8,
    d: bool,
    e: [i8; 2],
}

#[derive(Savefile, Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
enum Enum8 {
    A(u8, [u8; 2]),
    B([u8; 3]),
    C,
}

#[derive(Savefile, Debug, PartialEq, Clone, Copy)]
#[repr(u16)]
enum Fieldless16 {
    A,
    B,
}

#[derive(Savefile, Debug, PartialEq, Clone)]
struct MixedByteRun {
    a: u8,
    b: u8,
    c: [u8; 2],
    s: String,
    d: u32,
}

#[test]
fn byte_only_types_are_packed_on_any_host() {
    unsafe {
        assert!(<u8 as Packed>::repr_c_optimization_safe(0).is_yes());
        assert!(<[u8; 7] as Packed>::repr_c_optimization_safe(0).is_yes());
        assert!(<(u8, i8, bool) as Packed>::repr_c_optimization_safe(0).is_yes());
        assert!(<ByteStruct as Packed>::repr_c_optimization_safe(0).is_yes());
        assert!(<Enum8 as Packed>::repr_c_optimization_safe(0).is_yes());

        let little = cfg!(target_endian = "little");
        assert_eq!(<u16 as Packed>::repr_c_optimization_safe(0).is_yes(), little);
        assert_eq!(<PackedStruct as Packed>::repr_c_optimization_safe(0).is_yes(), little);
        assert_eq!(<Fieldless16 as Packed>::repr_c_optimization_safe(0).is_yes(), little);
    }
}

#[test]
fn expected_byte_struct() {
    let v = ByteStruct {
        a: 1,
        b: [2, 3, 4],
        c: -1,
        d: true,
        e: [5, -2],
    };
    assert_eq!(roundtrip(&v), vec![1, 2, 3, 4, 0xff, 1, 5, 0xfe]);
    let mut expected = vec![2, 0, 0, 0, 0, 0, 0, 0];
    expected.extend([1, 2, 3, 4, 0xff, 1, 5, 0xfe]);
    expected.extend([1, 2, 3, 4, 0xff, 1, 5, 0xfe]);
    assert_eq!(roundtrip(&vec![v, v]), expected);
}

#[test]
fn expected_enum8() {
    assert_eq!(roundtrip(&Enum8::A(1, [2, 3])), vec![0, 1, 2, 3]);
    assert_eq!(roundtrip(&Enum8::B([4, 5, 6])), vec![1, 4, 5, 6]);
    assert_eq!(roundtrip(&Enum8::C), vec![2]);
    assert_eq!(roundtrip(&vec![Fieldless16::B]), vec![1, 0, 0, 0, 0, 0, 0, 0, 1, 0]);
}

#[test]
fn expected_mixed_byte_run() {
    let v = MixedByteRun {
        a: 1,
        b: 2,
        c: [3, 4],
        s: "x".to_string(),
        d: 0x05060708,
    };
    assert_eq!(
        roundtrip(&v),
        vec![1, 2, 3, 4, 1, 0, 0, 0, 0, 0, 0, 0, b'x', 8, 7, 6, 5]
    );
}
