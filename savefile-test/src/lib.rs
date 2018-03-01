//! This is the documentation for `savefile`
//!
//! # Introduction
//!
//! Savefile is a rust library to conveniently, quickly and correctly
//! serialize and deserialize arbitrary rust struct and enums into
//! an efficient and compact binary version controlled format.
//!
//! The design use case is any application that needs to save large
//! amounts of data to disk, and support loading files from previous
//! versions of the program (but not from later versions!).
//!
//!
//! # Example
//!
//! ```
//! extern crate savefile;
//! use savefile::prelude::*;
//!
//! #[macro_use]
//! extern crate savefile_derive;
//! use std::fs::File;
//! use std::io::prelude::*;
//!
//!
//! #[derive(WithSchema,Serialize,Deserialize)]
//! struct Player {
//!     name : String,
//!     strength : u32,
//!     inventory : Vec<String>,
//! }
//!
//! fn save(player:&Player) {
//!     let mut f = File::create("save.bin").unwrap();
//!     Serializer::store(&mut f, 0, player);
//! }
//!
//! fn load() -> Player {
//!     let mut f = File::open("save.bin").unwrap();
//!     Deserializer::fetch(&mut f, 0)
//! }
//!
//! fn main() {
//!     save(&Player { name: "Steve".to_string(), strength: 42,
//!         inventory: vec!(
//!             "wallet".to_string(),
//!             "car keys".to_string(),
//!             "glasses".to_string())});
//!     assert_eq!(load().name,"Steve".to_string());
//!
//! }
//!
//! ```
//!

#![feature(test)]
extern crate test;
extern crate savefile;
#[macro_use]
extern crate savefile_derive;
use std::fmt::Debug;
use std::io::Write;
use savefile::prelude::*;

mod test_versioning;
mod test_nested_non_repr_c;
mod test_nested_repr_c;

#[derive(WithSchema, Debug, Serialize, Deserialize, PartialEq)]
struct NonCopy {
    ncfield: u8,
}

use std::io::Cursor;
use std::io::BufWriter;

pub fn assert_roundtrip<E: Serialize + Deserialize + Debug + PartialEq>(sample: E) {
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            let mut serializer = Serializer::store(&mut bufw, 0, &sample);
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    {
        let roundtrip_result = Deserializer::fetch::<E>(&mut f, 0);
        assert_eq!(sample, roundtrip_result);        
    }

    let f_internal_size = f.get_ref().len();
    assert_eq!(f.position() as usize,f_internal_size);
}

#[derive(WithSchema, Debug, Serialize, Deserialize, PartialEq)]
pub enum TestStructEnum {
    Variant1 { a: u8, b: u8 },
    Variant2 { a: u8 },
}

#[test]
pub fn test_struct_enum() {
    assert_roundtrip(TestStructEnum::Variant1 { a: 42, b: 45 });
    assert_roundtrip(TestStructEnum::Variant2 { a: 47 });
}

#[test]
pub fn test_tuple_enum() {
    #[derive(WithSchema, Debug, Serialize, Deserialize, PartialEq)]
    pub enum TestTupleEnum {
        Variant1(u8),
    }
    assert_roundtrip(TestTupleEnum::Variant1(37));
}

#[test]
pub fn test_unit_enum() {
    #[derive(WithSchema, Debug, Serialize, Deserialize, PartialEq)]
    pub enum TestUnitEnum {
        Variant1,
        Variant2,
    }
    assert_roundtrip(TestUnitEnum::Variant1);
    assert_roundtrip(TestUnitEnum::Variant2);
}

#[derive(Debug,WithSchema,  Serialize, Deserialize, PartialEq)]
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
    });
}

#[test]
pub fn test_vec() {
    let mut v = Vec::new();
    v.push(43u8);

    assert_roundtrip(v);
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

#[derive(ReprC, WithSchema, Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchStruct {
    x: usize,
    y: usize,
    z: u8,
}


use test::{Bencher, black_box};

#[bench]
fn bench_serialize(b: &mut Bencher) {

    let mut f = Cursor::new(Vec::with_capacity(100));

    let mut test=Vec::new();
    for i in 0..1000 {
    	test.push(BenchStruct {
    		x:black_box(i),
    		y:black_box(i),
    		z:black_box(0)
    	})
    }
 	b.iter(move || {
        {            
            let mut serializer = Serializer::store_noschema(&mut f,0,&test);
        }
        black_box(&mut f);

        f.set_position(0);
        {
            let r = Deserializer::fetch_noschema::<Vec<BenchStruct>>(&mut f, 0);            
            assert!(r.len()==1000);  
        }       

        f.set_position(0);
    });
}

#[test]
pub fn test_bench_struct() {
    assert_roundtrip(
        vec![
            BenchStruct {
                x:black_box(1),
                y:black_box(2),
                z:black_box(3)
            },
            BenchStruct {
                x:black_box(4),
                y:black_box(5),
                z:black_box(6)
            },
            BenchStruct {
                x:black_box(7),
                y:black_box(8),
                z:black_box(9)
            },
            BenchStruct {
                x:black_box(1),
                y:black_box(2),
                z:black_box(3)
            }
            ]
        );
}

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct SmallStruct {
    x1: u32,
    x2: i32,
}

#[test]
pub fn test_small_struct() {
    assert_roundtrip(SmallStruct { x1: 123, x2: 321 });
}

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct SmallStruct2 {
    x1: u32,
    x2: i32,
    #[default_val = "100"]
    #[versions = "1.."]
    x3: String,
    #[default_val = "123"]
    #[versions = "1.."]
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
            Serializer::store(&mut bufw, version_number1, &sample_v1);
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    let roundtrip_result = Deserializer::fetch::<E2>(&mut f, version_number2);    
    assert_eq!(expected_v2, roundtrip_result);
    roundtrip_result
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

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct SmallStructRem1 {
    x1: u32,
    x2: i32,
    x3: String,
}
#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct SmallStructRem2 {
    #[versions = "..0"]
    x1: Removed<u32>,
    x2: i32,
    x3: String,
    #[default_val = "123"]
    #[versions = "1.."]
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
