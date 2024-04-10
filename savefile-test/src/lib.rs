#![allow(unused_imports)]
#![cfg_attr(feature="nightly", feature(test))]
#![deny(warnings)]

#[cfg(test)]
extern crate insta;
extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

extern crate serde;
#[macro_use]
extern crate serde_derive;

#[cfg(feature="nightly")]
extern crate test;
extern crate savefile;
#[macro_use]
extern crate savefile_derive;

extern crate bit_vec;
extern crate bit_set;
extern crate smallvec;
extern crate byteorder;
extern crate rand;
extern crate indexmap;
extern crate rustc_hash;

use std::fmt::Debug;
use std::io::Write;
use savefile::prelude::*;
use indexmap::IndexMap;
use indexmap::IndexSet;
extern crate arrayvec;
extern crate parking_lot;
extern crate savefile_abi;
extern crate bincode;


mod test_versioning;
mod savefile_abi_test;
mod test_introspect;
mod test_nested_non_repr_c;
mod test_nested_repr_c;
mod test_arrayvec;
mod test_generic;

#[cfg(feature = "external_benchmarks")]
#[cfg(not(miri))]
mod ext_benchmark;


#[derive(Debug, Savefile, PartialEq)]
struct NonCopy {
    ncfield: u8,
}

use std::io::Cursor;
use std::io::BufWriter;

pub fn assert_roundtrip<E: Serialize + Deserialize + Debug + PartialEq>(sample: E) {
    assert_roundtrip_version(sample, 0, true)
}

pub fn assert_roundtrip_version<E: Serialize + Deserialize + Debug + PartialEq>(sample: E,version:u32, schema: bool) {
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            if schema {
                Serializer::save(&mut bufw, version, &sample, false).unwrap();
            } else {
                Serializer::save_noschema(&mut bufw, version, &sample).unwrap();
            }
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    {
        let roundtrip_result =
        if schema {
            Deserializer::load::<E>(&mut f, version).unwrap()
        } else {
            Deserializer::load_noschema::<E>(&mut f, version).unwrap()
        };
        assert_eq!(sample, roundtrip_result);
    }

    let f_internal_size = f.get_ref().len();
    assert_eq!(f.position() as usize,f_internal_size);
}

pub fn assert_roundtrip_debug<E: Serialize + Deserialize + Debug>(sample: E) {
    let sample_debug_string = format!("{:?}", sample);
    let round_tripped = roundtrip(sample);
    assert_eq!(
        sample_debug_string,
        format!("{:?}", round_tripped));
}

pub fn roundtrip<E: Serialize + Deserialize>(sample: E) -> E {
    roundtrip_version(sample, 0)
}
pub fn roundtrip_version<E: Serialize + Deserialize>(sample: E, version: u32) -> E {
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            Serializer::save(&mut bufw, version, &sample, false).unwrap();
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    let roundtrip_result;
    {
        roundtrip_result = Deserializer::load::<E>(&mut f, version).unwrap();
    }

    let f_internal_size = f.get_ref().len();
    assert_eq!(f.position() as usize,f_internal_size);
    roundtrip_result
}
#[derive(Debug, Savefile, PartialEq)]
pub enum TestStructEnum {
    Variant1 { a: u8, b: u8 },
    Variant2 { a: u8 },
}

#[test]
pub fn test_struct_enum() {
    assert_roundtrip(TestStructEnum::Variant1 { a: 42, b: 45 });
    assert_roundtrip(TestStructEnum::Variant2 { a: 47 });
}

#[derive(Debug, Savefile, PartialEq)]
pub enum TestTupleEnum {
    Variant1(u8),
}
#[test]
pub fn test_tuple_enum() {
    assert_roundtrip(TestTupleEnum::Variant1(37));
}

#[test]
pub fn test_unit_enum() {
    #[derive(Debug, Savefile, PartialEq)]
    pub enum TestUnitEnum {
        Variant1,
        Variant2,
    }
    assert_roundtrip(TestUnitEnum::Variant1);
    assert_roundtrip(TestUnitEnum::Variant2);
}

#[derive(Debug, Savefile, PartialEq)]
pub struct TestStruct {
    x1: u8,
    x2: u16,
    x3: u32,
    x4: u64,
    x5: usize,
    x6: i8,
    x7: i16,
    x8: i32,
    x9: i64,
    x10: isize,
    x11: f32,
    x12 : bool,
    x13: u128,
    x14: i128,
}

#[test]
pub fn test_struct_reg() {
    assert_roundtrip(TestStruct {
        x1: 1,
        x2: 2,
        x3: 3,
        x4: 4,
        x5: 5,
        x6: 6,
        x7: 7,
        x8: 8,
        x9: 9,
        x10: 10,
        x11 : 11.5,
        x12 : true,
        x13: 13,
        x14: -14,
    });
}

#[test]
pub fn test_vec() {
    let mut v = Vec::new();
    v.push(43u8);

    assert_roundtrip(v);
}


#[derive(Savefile,Debug,PartialEq)]
struct GenericWrapper<T:Serialize+Deserialize+WithSchema+Debug+PartialEq+Introspect> {
    something : T
}

#[test]
pub fn test_generic() {

    assert_roundtrip(GenericWrapper {
        something:42u32
    });
}

#[test]
pub fn test_bin_heap() {
    use std::collections::BinaryHeap;
    let mut v = BinaryHeap::new();
    v.push(43u8);

    let vv:Vec<u8>=v.iter().map(|x|*x).collect();
    let n=roundtrip(v);
    let nv:Vec<u8>=n.iter().map(|x|*x).collect();

    assert_eq!(nv,vv);
}

#[test]
pub fn test_vec_of_string() {
    let mut v = Vec::new();
    v.push("hejsan".to_string());

    assert_roundtrip(v);
}

#[test]
pub fn test_hashmap() {
    use std::collections::HashMap;
    let mut v = HashMap::new();
    v.insert(43, 45);
    v.insert(47, 49);

    assert_roundtrip(v);
}

#[test]
pub fn test_string() {
    assert_roundtrip("".to_string());
    assert_roundtrip("test string".to_string());
}


#[derive(Clone, Copy, Debug, Savefile, PartialEq)]
#[savefile_unsafe_and_fast]
pub struct BenchStruct {
    x: usize,
    y: usize,
    z: u8,
    pad1:u8,
    pad2:u8,
    pad3:u8,
    pad4:u32,
}

#[cfg(feature="nightly")]
#[cfg(not(miri))]
use test::{Bencher, black_box};

#[derive(Savefile,PartialEq,Eq,Clone,Debug)]
struct StructWithArrayString{
    arraystr: ArrayString<30>
}


#[test]
pub fn test_struct_with_arraystring() {
    assert_roundtrip(StructWithArrayString {
        arraystr: "hej".try_into().unwrap()
    });
}


#[cfg(feature="nightly")]
#[bench]
#[cfg(not(miri))]
fn bench_savefile_serialize(b: &mut Bencher) {

    let mut f = Cursor::new(Vec::with_capacity(100));

    let mut test=Vec::new();
    for i in 0..1000 {
    	test.push(BenchStruct {
    		x:black_box(i),
    		y:black_box(i),
    		z:black_box(0),
            pad1:0,
            pad2:0,
            pad3:0,
            pad4:0,
    	})
    }
 	b.iter(move || {
        {
            save_noschema(&mut f,0,&test).unwrap();
        }
        black_box(&mut f);

        f.set_position(0);
        {
            let r = load_noschema::<Vec<BenchStruct>>(&mut f, 0).unwrap();
            assert!(r.len()==1000);
        }

        f.set_position(0);
    });
}

#[cfg(feature="nightly")]
#[test]
#[cfg(not(miri))]
pub fn test_bench_struct() {
    assert_roundtrip(
        vec![
            BenchStruct {
                x:black_box(1),
                y:black_box(2),
                z:black_box(3),
                pad1:0,pad2:0,pad3:0,pad4:0,
            },
            BenchStruct {
                x:black_box(4),
                y:black_box(5),
                z:black_box(6),
                pad1:0,pad2:0,pad3:0,pad4:0,
            },
            BenchStruct {
                x:black_box(7),
                y:black_box(8),
                z:black_box(9),
                pad1:0,pad2:0,pad3:0,pad4:0,
            },
            BenchStruct {
                x:black_box(1),
                y:black_box(2),
                z:black_box(3),
                pad1:0,pad2:0,pad3:0,pad4:0,
            }
        ]
    );
}


#[test]
pub fn test_bench_struct_miri_compat() {
    assert_roundtrip(
        vec![
            BenchStruct {
                x:1,
                y:2,
                z:3,
                pad1:0,pad2:0,pad3:0,pad4:0,
            },
            BenchStruct {
                x:4,
                y:5,
                z:6,
                pad1:0,pad2:0,pad3:0,pad4:0,
            },
            BenchStruct {
                x:7,
                y:8,
                z:9,
                pad1:0,pad2:0,pad3:0,pad4:0,
            },
            BenchStruct {
                x:10,
                y:11,
                z:12,
                pad1:0,pad2:0,pad3:0,pad4:0,
            }
        ]
    );
}
#[test]
pub fn test_u16_vec() {
    assert_roundtrip(Vec::<u16>::new());
    assert_roundtrip(vec![0u16,42u16]);
    assert_roundtrip(vec![0u16,1,2,3,4,5,6,7,8,9]);
}

#[derive(Debug, PartialEq, Savefile)]
pub struct SmallStruct {
    x1: u32,
    x2: i32,
}

#[derive(Debug, PartialEq, Savefile)]
pub struct NotSoSmallStruct {
    x1: u32,
    x2: bool,
    x3: u32,
    x4: i32,
}

pub fn serialize_small_struct(input: NotSoSmallStruct, output: &mut Serializer<Vec<u8>>) {
    let _ = input.serialize(output);
}
pub fn serialize_small_struct_manual(input: NotSoSmallStruct, output: &mut Vec<u8>) {
    let slice: [u8;std::mem::size_of::<NotSoSmallStruct>()] = unsafe {std::mem::transmute(input)};
    output.extend(slice);
}

#[test]
pub fn test_small_struct() {
    assert_roundtrip(SmallStruct { x1: 123, x2: 321 });
}

#[derive(Debug, PartialEq, Savefile)]
struct SmallStruct2 {
    x1: u32,
    x2: i32,
    #[savefile_default_val = "100"]
    #[savefile_versions = "1.."]
    x3: String,
    #[savefile_default_val = "123"]
    #[savefile_versions = "1.."]
    x4: u64,
}

pub fn assert_roundtrip_to_new_version<
    E1: Serialize + Deserialize + Debug + PartialEq,
    E2: Serialize + Deserialize + Debug + PartialEq,
> (
    sample_v1: E1,
    version_number1: u32,
    expected_v2: E2,
    version_number2: u32,
) -> E2 {
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            Serializer::save(&mut bufw, version_number1, &sample_v1, false).unwrap();
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    let roundtrip_result = Deserializer::load::<E2>(&mut f, version_number2).unwrap();
    assert_eq!(expected_v2, roundtrip_result);
    roundtrip_result
}

#[test]
pub fn test_array_string() {
    use arrayvec::ArrayString;
    let arraystr:ArrayString<30>=ArrayString::from("Hello everyone").unwrap();
    assert_roundtrip(arraystr);
}

#[test]
pub fn test_array_vec() {
    use arrayvec::ArrayVec;
    let mut data:ArrayVec<u32, 30> = ArrayVec::new();
    assert_roundtrip(data.clone());
    data.push(47);
    assert_roundtrip(data.clone());
    data.push(1);
    data.push(32);
    data.push(49);
    assert_roundtrip(data.clone());
}

#[test]
pub fn test_array_vec_with_string() {
    use arrayvec::ArrayVec;
    let mut data:ArrayVec<String, 30> = ArrayVec::new();
    assert_roundtrip(data.clone());
    data.push("hello".to_string());
    assert_roundtrip(data.clone());
    data.push("wonderful".to_string());
    data.push("world".to_string());
    data.push("how ya doing?".to_string());
    assert_roundtrip(data.clone());
}

#[test]
pub fn test_smallvec0() {
    let mut v = smallvec::SmallVec::<[u8;2]>::new();
    v.push(1);
    assert_roundtrip(v);
}

#[test]
pub fn test_smallvec1() {
    let mut v = smallvec::SmallVec::<[u8;2]>::new();
    v.push(1);
    assert_roundtrip(v);
}

#[test]
pub fn test_smallvec2() {
    let mut v = smallvec::SmallVec::<[u8;2]>::new();
    v.push(1);
    v.push(2);
    assert_roundtrip(v);
}

#[test]
pub fn test_smallvec3() {
    let mut v = smallvec::SmallVec::<[u8;2]>::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert_roundtrip(v);
}




#[test]
pub fn test_short_arrays() {
    let empty:[u32;0]=[];
    assert_roundtrip(empty);
    assert_roundtrip([1]);
    assert_roundtrip([1,2]);
    assert_roundtrip([1,2,3]);
}


#[test]
pub fn test_short_array_with_drop_contents() {
    let empty:[String;0]=[];
    assert_roundtrip(empty);
    assert_roundtrip(["Hej".to_string(),"Hello".to_string()]);
}

#[test]
pub fn test_short_array_with_drop_contents_leak_test() {
    let mut i =0;
    loop {
        let test = [format!("Test {}",i),format!("Other {}",i)];
        assert_roundtrip(test);
        i+=1;
        if i>23 {
            break;
        }
    }
}
#[test]
pub fn test_string_leak_test() {
    let mut i =0;
    loop {
        let test = format!("Test {}",i);
        assert_roundtrip(test);
        i+=1;
        if i>23 {
            break;
        }
    }
}


#[cfg(feature="nightly")]
#[test]
pub fn test_long_array() {
    let arr=[47;32];
    assert_roundtrip(arr);
}


#[cfg(feature="nightly")]
#[test]
pub fn test_very_long_array() {
    #[derive(Savefile)]
    struct LongArray([u32;1000]);
    impl Debug for LongArray {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "Long array")
        }
    }
    impl PartialEq for LongArray {
        fn eq(&self, other: &Self) -> bool {
            for idx in 0..1000 {
                if self.0[idx] != other.0[idx] {
                    return false;
                }
            }
            true
        }
    }

    let mut arr=LongArray([47;1000]);
    arr.0[0]=0;
    assert_roundtrip(arr);
}

#[test]
pub fn test_small_struct_upgrade() {
    assert_roundtrip_to_new_version(
        SmallStruct { x1: 123, x2: 321 },
        0,
        SmallStruct2 {
            x1: 123,
            x2: 321,
            x3: "100".to_string(),
            x4: 123,
        },
        1,
    );
}

#[derive(Debug, PartialEq, Savefile)]
struct SmallStructRem1 {
    x1: u32,
    x2: i32,
    x3: String,
}
#[derive(Debug, PartialEq, Savefile )]
struct SmallStructRem2 {
    #[savefile_versions = "..0"]
    x1: Removed<u32>,
    x2: i32,
    x3: String,
    #[savefile_default_val = "123"]
    #[savefile_versions = "1.."]
    x4: isize,
}

#[test]
pub fn test_small_struct_remove() {
    assert_roundtrip_to_new_version(
        SmallStructRem1 {
            x1: 123,
            x2: 321,
            x3: "hello".to_string(),
        },
        0,
        SmallStructRem2 {
            x1: Removed::new(),
            x2: 321,
            x3: "hello".to_string(),
            x4: 123,
        },
        1,
    );
}



#[derive(Debug, PartialEq, Savefile )]
struct TupleCarrier {
    t0 : (),
    t1 : (u32,),
    t2 : (u32,u32),
    t3 : (u32,u32,u32),
}

#[test]
pub fn test_tuple() {
     assert_roundtrip(TupleCarrier{
        t0:(),
        t1:(42u32,),
        t2:(42u32,43u32),
        t3:(42u32,43u32,44u32),
    });
}

#[derive(Debug, PartialEq, Savefile )]
struct StructWithIgnored {
    a:u32,
    b:u32,
    #[savefile_ignore]
    c:u32,
}

#[test]
pub fn test_ignored() {
    assert_roundtrip(StructWithIgnored{a:42,b:7,c:0});
}


#[test]
pub fn test_box() {
    use std::rc::Rc;
    use std::sync::Arc;
    use std::cell::RefCell;
    use std::cell::Cell;
    assert_roundtrip(Box::new(37));
    assert_roundtrip(Rc::new(38));
    assert_roundtrip(Arc::new(39));
    assert_roundtrip(RefCell::new(40));
    assert_roundtrip(Cell::new(40));
}
#[test]
pub fn test_option() {
    assert_roundtrip(Some(32));
    let x:Option<u32> = None;
    assert_roundtrip(x);
}

#[test]
pub fn test_result() {
    let x:Result<u32,u32> = Ok(33);
    assert_roundtrip(x);
    let x:Result<u32,u32> = Err(33);
    assert_roundtrip(x);
}

#[derive(Savefile,Debug,PartialEq)]
struct NewTypeSample(u32);

#[test]
pub fn test_newtype() {
    assert_roundtrip(NewTypeSample(43));
}

#[derive(Savefile,Debug,PartialEq)]
struct NewTypeSample2(u32,i8);

#[test]
pub fn test_newtype2() {

    assert_roundtrip(NewTypeSample2(43,127));

}

#[derive(Savefile,Debug,PartialEq)]
struct NoFields {
}

#[test]
pub fn test_struct_no_fields() {
    assert_roundtrip(NoFields{});
}


#[derive(Savefile,Debug,PartialEq)]
struct OnlyRemoved {
    #[savefile_versions="0..0"]
    rem : Removed<u32>,
}

#[test]
pub fn test_struct_only_removed_fields() {
    assert_roundtrip_version(OnlyRemoved{rem: Removed::new()},1, true);
}



#[test]
pub fn test_bitvec() {
    use bit_vec::BitVec;
    let bv1 = BitVec::new();
    let mut bv2 = BitVec::new();
    bv2.push(false);
    let mut bv3 = BitVec::new();
    bv3.push(false);
    bv3.push(true);
    bv3.push(false);
    let mut bv4 = BitVec::new();
    for i in 0..127 {
        bv4.push(if i%2==0 {true} else {false});
    }
    let mut bv5 = BitVec::new();
    for i in 0..127 {
        bv5.push(if i%3==0 {true} else {false});
    }
    assert_roundtrip(bv1);
    assert_roundtrip(bv2);
    assert_roundtrip(bv3);
    assert_roundtrip(bv4);
    assert_roundtrip(bv5);
}

#[test]
pub fn test_bitset() {
    use bit_set::BitSet;

    let bs1 = BitSet::new();
    assert_roundtrip(bs1);

    let mut bs2 = BitSet::new();
    bs2.insert(0);
    assert_roundtrip(bs2);
    let mut bs3 = BitSet::new();
    bs3.insert(0);
    bs3.insert(3);
    bs3.insert(7);
    assert_roundtrip(bs3);

    let mut bs4 = BitSet::new();
    bs4.insert(0);
    bs4.insert(3);
    bs4.insert(200);
    assert_roundtrip(bs4);

}
#[repr(u8)]
#[derive(Savefile, Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[savefile_unsafe_and_fast]
pub enum TerrainType {
    Wheat,
    Forest,
    Desert,
    Rock,
    Dirt,
    Grass,
    Water,
}

#[repr(C)]
#[derive(Savefile, Clone, Copy, Debug,PartialEq)]
#[savefile_unsafe_and_fast]
pub struct TerrainTile
{
    pub curtype: TerrainType,
    pub resource: u8, //logarithmic scale, base resource abundance
    pub height: i16,
}


#[test]
pub fn test_terrain() {
    assert_roundtrip(vec![TerrainTile {
        curtype : TerrainType::Dirt,
        resource:42,
        height:2111
    }]);
}


#[cfg(test)]
use std::sync::atomic::{AtomicU8,AtomicUsize,Ordering};
use std::string::ToString;
use savefile::{diff_schema, save_compressed, VecOrStringLayout};
use std::sync::Arc;
use std::path::PathBuf;
use smallvec::alloc::collections::BTreeMap;
use std::collections::HashSet;
use std::borrow::Cow;
use std::convert::TryInto;
use std::sync::atomic::{AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicU16, AtomicU32, AtomicU64};
use std::time::Instant;
use arrayvec::ArrayString;
use quickcheck::{Arbitrary, Gen};
use rustc_hash::{FxHashMap, FxHashSet};

#[test]
pub fn test_atomic() {
    let atom = AtomicU8::new(43);
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            Serializer::save(&mut bufw, 1, &atom, false).unwrap();
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    {
        let roundtrip_result : AtomicU8 = Deserializer::load(&mut f, 1).unwrap();
        assert_eq!(atom.load(Ordering::SeqCst), roundtrip_result.load(Ordering::SeqCst));
    }
}

#[test]
pub fn test_all_atomics() {
    assert_roundtrip_debug(AtomicU8::new(42));
    assert_roundtrip_debug(AtomicU16::new(42));
    assert_roundtrip_debug(AtomicU32::new(42));
    assert_roundtrip_debug(AtomicU64::new(42));
    assert_roundtrip_debug(AtomicI8::new(42));
    assert_roundtrip_debug(AtomicI16::new(42));
    assert_roundtrip_debug(AtomicI32::new(42));
    assert_roundtrip_debug(AtomicI64::new(42));
    assert_roundtrip_debug(AtomicIsize::new(42));
    assert_roundtrip_debug(AtomicUsize::new(42));
}


#[test]
pub fn test_schema1()  {
    assert_roundtrip_version(
        Schema::Vector(Box::new(Schema::Primitive(SchemaPrimitive::schema_u32)), VecOrStringLayout::CapacityDataLength),
        1, false
    );
    assert_roundtrip_version(
        Schema::Vector(Box::new(Schema::Primitive(SchemaPrimitive::schema_string(VecOrStringLayout::DataCapacityLength))), VecOrStringLayout::CapacityDataLength),
        1, false
    );
}
#[test]
pub fn test_schema2()  {
    assert_roundtrip_version(
        Schema::Vector(Box::new(Schema::Primitive(SchemaPrimitive::schema_string(VecOrStringLayout::DataCapacityLength))), VecOrStringLayout::CapacityDataLength),
        1, false
    );
}

#[derive(Savefile,Debug,PartialEq)]
struct CanaryTest {
    canary1: Canary1,
    some_field: i32
}

#[test]
pub fn test_canary1() {
    assert_roundtrip(CanaryTest{
        canary1: Canary1::default(),
        some_field : 43
    });
}




#[test]
#[cfg(not(miri))]
pub fn test_crypto1() {
    use byteorder::{LittleEndian};
    use byteorder::WriteBytesExt;
    use byteorder::ReadBytesExt;

    let zerokey = [0u8;32];
    let mut temp = Vec::new();
    {
        let mut writer = CryptoWriter::new(&mut temp,zerokey).unwrap();
        writer.write_u32::<LittleEndian>(0x01020304).unwrap();
        writer.flush().unwrap();
    }
    let zerokey = [0u8;32];

    let mut bufr = std::io::BufReader::new(&temp[..]);
    let mut reader = CryptoReader::new(&mut bufr, zerokey).unwrap();

    let end = reader.read_u32::<LittleEndian>().unwrap();

    assert_eq!(end,0x01020304);
}

#[test]
#[cfg(not(miri))]
pub fn test_compressed_big() {
    let mut zeros = Vec::new();
    for _ in 0..100_000 {
        zeros.push(0);
    }
    let mut buf = Vec::new();
    save_compressed(&mut buf, 0, &zeros).unwrap();

    assert!(buf.len() < 100);

    let roundtripped :Vec<i32> = load(&mut Cursor::new(&buf), 0).unwrap();

    assert_eq!(zeros,roundtripped);

}


#[test]
#[cfg(not(miri))]
pub fn test_compressed_small() {

    let input = 42u8;
    let mut buf = Vec::new();
    save_compressed(&mut buf, 0, &input).unwrap();
    let mut bufp = &buf[..];
    let roundtripped :u8 = load(&mut bufp, 0).unwrap();

    assert_eq!(input,roundtripped);
}
#[test]
#[cfg(not(miri))]
pub fn test_compressed_smallish() {

    let input = 42u64;
    let mut buf = Vec::new();
    save_compressed(&mut buf, 0, &input).unwrap();
    let mut bufp = &buf[..];
    let roundtripped :u64 = load(&mut bufp, 0).unwrap();

    assert_eq!(input,roundtripped);
}

#[test]
#[cfg(not(miri))]
pub fn test_crypto_big1() {
    use byteorder::{LittleEndian};
    use byteorder::WriteBytesExt;
    use byteorder::ReadBytesExt;

    let zerokey = [0u8;32];
    let mut temp = Vec::new();

    let mut writer = CryptoWriter::new(&mut temp,zerokey).unwrap();
    for i in 0..10000 {
        writer.write_u64::<LittleEndian>(i).unwrap();
    }
    writer.flush_final().unwrap();

    let zerokey = [0u8;32];

    let mut bufr = std::io::BufReader::new(&temp[..]);
    let mut reader = CryptoReader::new(&mut bufr, zerokey).unwrap();

    for i in 0..10000 {
        assert_eq!(reader.read_u64::<LittleEndian>().unwrap(),i);
    }
}



#[test]
#[cfg(not(miri))]
pub fn test_crypto_big2() {
    use byteorder::{LittleEndian};
    use byteorder::WriteBytesExt;
    use byteorder::ReadBytesExt;

    let zerokey = [0u8;32];
    let mut kb = [0u8;1024];
    let mut temp = Vec::new();
    {
        let mut writer = CryptoWriter::new(&mut temp,zerokey).unwrap();
        let kbl = kb.len();
        for i in 0..kbl {
            kb[i] = (i/4) as u8;
        }
        for _ in 0..1000 {
            writer.write(&kb).unwrap();
        }
        writer.flush().unwrap();

    }
    let zerokey = [0u8;32];

    let mut bufr = std::io::BufReader::new(&temp[..]);

    let mut reader = CryptoReader::new(&mut bufr, zerokey).unwrap();

    use std::io::Read;
    let mut testkb= [0;1024];
    for _ in 0..1000 {
        reader.read_exact(&mut testkb).unwrap();
        for j in 0..kb.len() {

            assert_eq!(kb[j],testkb[j]);
        }
    }
}


#[test]
#[cfg(not(miri))]
pub fn test_crypto_big3() {
    use byteorder::{LittleEndian};
    use byteorder::WriteBytesExt;
    use byteorder::ReadBytesExt;

    let zerokey = [0u8;32];
    let mut kb = [0u8;1024*128-17];
    let mut temp = Vec::new();
    {
        let mut writer = CryptoWriter::new(&mut temp,zerokey).unwrap();
        let kbl = kb.len();
        for i in 0..kbl {
            kb[i] = (i/4) as u8;
        }
        for _ in 0..10 {
            writer.write(&kb).unwrap();
        }
        writer.flush().unwrap();

    }
    let zerokey = [0u8;32];

    let mut bufr = std::io::BufReader::new(&temp[..]);

    let mut reader = CryptoReader::new(&mut bufr, zerokey).unwrap();

    use std::io::Read;
    let mut testkb= [0;1024*128-17];
    for _ in 0..10 {
        reader.read_exact(&mut testkb).unwrap();
        for j in 0..kb.len() {

            assert_eq!(kb[j],testkb[j]);
        }
    }
}


#[test]
#[cfg(not(miri))]
pub fn test_crypto_big4() {
    use byteorder::{LittleEndian};
    use byteorder::WriteBytesExt;
    use byteorder::ReadBytesExt;

    let zerokey = [0u8;32];
    let mut kb = [0u8;10000];
    let mut temp = Vec::new();
    {
        let mut writer = CryptoWriter::new(&mut temp,zerokey).unwrap();
        let kbl = kb.len();
        for i in 0..kbl {
            kb[i] = (i/4) as u8;
        }
        for _ in 0..1000 {
            writer.write(&kb).unwrap();
        }
        writer.flush().unwrap();

    }
    let zerokey = [0u8;32];

    let mut bufr = std::io::BufReader::new(&temp[..]);

    let mut reader = CryptoReader::new(&mut bufr, zerokey).unwrap();

    use std::io::Read;
    let mut testkb= [0;10000];
    for _ in 0..1000 {
        reader.read_exact(&mut testkb).unwrap();
        for j in 0..kb.len() {

            assert_eq!(kb[j],testkb[j]);
        }
    }
}
#[test]
#[cfg(not(miri))]
pub fn test_crypto_big5() {
    use byteorder::{LittleEndian};
    use byteorder::WriteBytesExt;
    use byteorder::ReadBytesExt;

    let mut kb = Box::new([0u8;1024*302]);
    let mut testkb = Box::new([0;1024*302]);

    for i in 0..kb.len() {
        kb[i] = (i%257) as u8;
    }
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let zerokey = [0u8;32];
    let mut temp = Vec::new();
    {
        let mut writer = CryptoWriter::new(&mut temp,zerokey).unwrap();
        let _kbl = kb.len();
        let mut offset = 0;
        loop {
            let mut delta:usize;
            if rng.gen_range(0..10) == 0 {
                delta = rng.gen_range(0..300_000);
            } else {
                delta = rng.gen_range(0..80000);
            }
            if delta + offset > kb.len() {
                delta = kb.len() - offset;
            }
            if delta == 0 {
                break;
            }
            writer.write(&kb[offset..offset+delta]).unwrap();
            offset += delta;
        }

        writer.flush().unwrap();

    }
    let zerokey = [0u8;32];

    let mut bufr = std::io::BufReader::new(&temp[..]);

    let mut reader = CryptoReader::new(&mut bufr, zerokey).unwrap();

    use std::io::Read;
    {

        let mut offset = 0;
        loop {
            let mut delta:usize;
            if rng.gen_range(0..10) == 0 {
                delta = rng.gen_range(0..300_000);
            } else {
                delta = rng.gen_range(0..80000);
            }
            if delta + offset > kb.len() {
                delta = kb.len() - offset;
            }
            if delta == 0 {
                break;
            }
            reader.read_exact(&mut testkb[offset..offset+delta]).unwrap();
            for i in offset..offset+delta {
                assert_eq!(testkb[i],kb[i]);
            }
            offset += delta;
        }
    }
}

#[test]
#[cfg(not(miri))]
pub fn test_encrypted_file1() {
    save_encrypted_file("test.bin",1,&47usize,"mypassword").unwrap();
    let result : usize = load_encrypted_file("test.bin",1,"mypassword").unwrap();
    assert_eq!(result,47usize);
}

#[test]
#[cfg(not(miri))]
pub fn test_encrypted_file_bad_password() {
    save_encrypted_file("test2.bin",1,&47usize,"mypassword").unwrap();
    let result = load_encrypted_file::<usize,_>("test2.bin",1,"mypassword2");
    assert!(result.is_err());
}

#[test]
#[cfg(not(miri))]
pub fn test_decrypt_junk_file() {
    {
        use std::fs::File;
        use byteorder::WriteBytesExt;
        use rand::Rng;
        let mut f = File::create("test3.bin").unwrap();
        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            f.write_u8(rng.gen()).unwrap();
        }
    }
    let result = load_encrypted_file::<usize,_>("test3.bin",1,"mypassword2");
    assert!(result.is_err());
}

#[derive(Savefile)]
struct MySimpleFuzz1 {
    integer: i8,
    strings: Vec<String>,
}

#[test]
pub fn fuzz_regression1() {
    let mut data:&[u8] = &[0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 8, 3, 0, 3, 0, 64, 0, 0, 0];
    let _t:Result<MySimpleFuzz1,_> = load_noschema(&mut data,0);
}

#[test]
pub fn fuzz_regression2() {
    let mut data:&[u8] = &[0, 0, 0, 0, 3, 11, 0, 254, 2, 1, 252, 255, 254];
    let _t:Result<MySimpleFuzz1,_> = load_noschema(&mut data,0);
}


#[test]
pub fn test_roundtrip_arc_slice() {
    let a1: Arc<[u32]> = vec![1,2,3,4].into();
    assert_roundtrip(a1);
    let a2: Arc<[String]> = vec!["Hello".to_string()].into();
    assert_roundtrip(a2);
}

#[test]
pub fn test_roundtrip_boxed_slice() {
    let a1: Box<[u32]> = vec![1,2,3,4].into_boxed_slice();
    assert_roundtrip(a1);
    let a2: Box<[String]> = vec!["Hello".to_string()].into_boxed_slice();
    assert_roundtrip(a2);
}

#[test]
pub fn test_serialize_btreemap() {
    let mut bm = BTreeMap::new();
    bm.insert(45,32u16);
    assert_roundtrip(bm);
}

#[test]
pub fn test_serialize_hashset() {
    let hs = HashSet::<i32>::new();
    assert_roundtrip(hs);
    let mut hs = HashSet::new();
    hs.insert(32u16);
    assert_roundtrip(hs);
    let mut hs = HashSet::new();
    hs.insert("hej".to_string());
    hs.insert("san".to_string());
    hs.insert("kompis".to_string());
    assert_roundtrip(hs);


}

#[test]
pub fn test_roundtrip_char() {

    assert_roundtrip('\r');
    assert_roundtrip('\n');
    assert_roundtrip('H');
    assert_roundtrip(',');
    assert_roundtrip('\0');
    assert_roundtrip('\u{10FFFF}');
}

#[test]
pub fn test_pathbuf() {
    let x: PathBuf = "/c/hello.txt".into();
    assert_roundtrip(x);

}

#[test]
pub fn test_arc_str() {
    let x:Arc<str> = "hej".into();
    assert_roundtrip(x);
}
#[test]
pub fn test_arc_str_dedup() {
    let x:Arc<str> = "hej".into();
    let y:Arc<str> = "hejsan".into();
    let z:Arc<str> = "hej".into();

    let (nx,ny,nz) = roundtrip((x.clone(),y.clone(),z.clone()));
    assert_ne!(nx.as_ptr(), x.as_ptr());
    assert_ne!(ny.as_ptr(), y.as_ptr());
    assert_ne!(nz.as_ptr(), z.as_ptr());
    assert_eq!(nx,nz);
    assert_ne!(nx,ny);
    assert_ne!(ny,nz);


}

#[test]
pub fn test_cow_owned() {
    let x:Cow<String> = Cow::Owned("hej".to_string());
    assert_roundtrip(x);
}

#[test]
pub fn test_cow_borrowed() {
    let borrow = "world".to_string();
    let x:Cow<String> = Cow::Borrowed(&borrow);
    assert_roundtrip(x);
}

#[derive(Savefile, Debug, PartialEq)]
struct SomethingWithPathbufIn {
    my_pathbuf: PathBuf
}

#[test]
pub fn test_pathbuf2() {
    let x  = SomethingWithPathbufIn {
        my_pathbuf: "/d/something.txt".into()
    };
    assert_roundtrip(x);
}

#[derive(SavefileNoIntrospect,Debug,PartialEq)]
struct ExampleWithoutAutomaticIntrospect {
    x: u32
}
impl Introspect for ExampleWithoutAutomaticIntrospect {
    fn introspect_value(&self) -> String {
        "Example".into()
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
}

#[cfg(test)]
fn speed(len:usize, time: std::time::Duration) -> String {
    let total_seconds = time.as_micros() as f64 * 1e-6;
    format!("{} bytes in {:.2}s = {} GB/sec",
        len, total_seconds,
            (len as f64/total_seconds)/1e9
    )
}

#[test]
#[ignore] //A bit expensive to run in CI
pub fn test_many_strings() {
    let mut outer:Vec<Vec<String>> = Vec::new();
    for _ in 0..100 {
        let mut inner = vec![];
        for _ in 0..1000_000 {
            inner.push("Test thing".into());
        }
        outer.push(inner);
    }
    let mut f = Cursor::new(Vec::with_capacity(2_000_000_000));
    {
        let t = Instant::now();
        Serializer::save_noschema(&mut f, 1, &outer).unwrap();
        println!("Save-Time: {}", speed(f.get_ref().len(),t.elapsed()));
    }
    {
        let t = Instant::now();
        f.set_position(0);
        let deserialized : Vec<Vec<String>> = Deserializer::load_noschema(&mut f, 1).unwrap();
        println!("Load-Time: {} (last value: {})",speed(f.get_ref().len(),t.elapsed()), deserialized.last().unwrap().last().unwrap());
    }
    println!("Size: {}", f.get_ref().len() as f64 / 1e9f64);
}


#[test]
#[ignore] //A bit expensive to run in CI
pub fn test_many_arraystrings() {
    let mut outer:Vec<Vec<ArrayString<20>>> = Vec::new();
    for _ in 0..100 {
        let mut inner = vec![];
        for _ in 0..1000_000 {
            inner.push("Test thing".try_into().unwrap());
        }
        outer.push(inner);
    }
    let mut f = Cursor::new(Vec::with_capacity(2_000_000_000));
    {
        let t = Instant::now();
        Serializer::save_noschema(&mut f, 1, &outer).unwrap();
        println!("Save-Time: {}", speed(f.get_ref().len(),t.elapsed()));
    }
    {
        let t = Instant::now();
        f.set_position(0);
        let deserialized : Vec<Vec<ArrayString<20>>> = Deserializer::load_noschema(&mut f, 1).unwrap();
        println!("Load-Time: {} (last value: {})",  speed(f.get_ref().len(),t.elapsed()), deserialized.last().unwrap().last().unwrap());
    }

    println!("Size: {}", f.get_ref().len() as f64 / 1e9f64);
}


#[test]
pub fn test_fx_hashmap() {
    let mut h = FxHashMap::default();
    h.insert(43u32,43u64);
    assert_roundtrip(h);
    assert_roundtrip(FxHashMap::<u32,u32>::default());
}
#[test]
pub fn test_fx_hashset() {
    let mut h = FxHashSet::default();
    h.insert(43u32);
    assert_roundtrip(h);
    assert_roundtrip(FxHashSet::<u32>::default());
}

#[derive(Savefile,Debug,PartialEq)]
struct MyUnitStruct;

#[test]
pub fn test_unit_struct() {
    let h = MyUnitStruct;
    assert_roundtrip(h);
}

#[test]
pub fn test_zero_size_vec_items() {
    let mut test = Vec::new();
    assert_roundtrip(test.clone());
    test.push(());
    assert_roundtrip(test.clone());
    test.push(());
    test.push(());
    test.push(());
    test.push(());
    test.push(());
    assert_roundtrip(test);
}


#[derive(Savefile,PartialEq,Debug)]
struct TestIgnoreExample {
    a: f64,
    b: f64,
    #[savefile_ignore]
    cached_product: f64
}

#[test]
pub fn test_struct_with_ignored_member() {
    assert_roundtrip(TestIgnoreExample{
        a:42.0,
        b:43.0,
        cached_product: 0.0,
    });
}

#[test]
pub fn test_indexmap() {
    let mut imap = IndexMap::new();
    assert_roundtrip(imap.clone());
    imap.insert(43u32,"hej".to_string());
    assert_roundtrip(imap.clone());

    imap.insert(44,"hej".to_string());
    imap.insert(45,"hej".to_string());
    assert_roundtrip(imap.clone());
}


#[test]
pub fn test_indexset() {
    let mut iset = IndexSet::new();
    assert_roundtrip(iset.clone());
    iset.insert((43u32,44u32));
    assert_roundtrip(iset.clone());

    iset.insert((43,43));
    iset.insert((44,44));
    assert_roundtrip(iset.clone());
}

#[cfg(test)]
struct RawStruct {
    a: u64,
    b: u32,
    c: u32,
}
#[test]
pub fn test_raw_write_region() {
    let mut data = vec![];
    let mut ser = Serializer {
        writer: &mut data,
        file_version: 0,
    };
    let r = RawStruct {
        a: 0, b:0, c: 42
    };
    let _ = r.c;
    unsafe{
        ser.raw_write_region(&r,&r.a, &r.b, 0).unwrap();
    }
}

#[quickcheck]
#[cfg(not(miri))]
fn test_quickcheck_roundtrip_simple_vec(xs: Vec<isize>) -> bool {
    xs == roundtrip(xs.clone())
}
#[quickcheck]
#[cfg(not(miri))]
fn test_quickcheck_roundtrip_hashset(xs: FxHashSet<String>) -> bool {
    println!("Yeah: {:?}", xs);
    xs == roundtrip(xs.clone())
}




#[ignore]
#[quickcheck]
#[cfg(not(miri))]
fn test_quickcheck_schema_roundtrip(a: Schema) -> bool {
    println!("Schema: {}", format!("{:?}",a).len());
    assert_roundtrip_version(a, 1, false);
    true
}

#[ignore]
#[quickcheck]
#[cfg(not(miri))]
fn test_quickcheck_schema_diff_different(a: Schema, b: Schema) -> bool {
    _ = diff_schema(&a, &b, "".into()); //Check this doesn't crash
    true
}
#[ignore]
#[quickcheck]
#[cfg(not(miri))]
fn test_quickcheck_schema_diff_same(a: Schema) -> bool {
    diff_schema(&a, &a, "".into()).is_none() //Should always equal itself
}
