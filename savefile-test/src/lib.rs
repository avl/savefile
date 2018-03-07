
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

#[derive(Debug, Savefile, PartialEq)]
struct NonCopy {
    ncfield: u8,
}

use std::io::Cursor;
use std::io::BufWriter;

pub fn assert_roundtrip<E: Serialize + Deserialize + Debug + PartialEq>(sample: E) {
    assert_roundtrip_version(sample, 0)
}
pub fn assert_roundtrip_version<E: Serialize + Deserialize + Debug + PartialEq>(sample: E,version:u32) {
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            Serializer::save(&mut bufw, version, &sample).unwrap();
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    {
        let roundtrip_result = Deserializer::load::<E>(&mut f, version).unwrap();
        assert_eq!(sample, roundtrip_result);        
    }

    let f_internal_size = f.get_ref().len();
    assert_eq!(f.position() as usize,f_internal_size);
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

#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate bincode;

#[derive(Serialize, Deserialize, ReprC, Clone, Copy, Debug, Savefile, PartialEq)]
pub struct BenchStruct {
    x: usize,
    y: usize,
    z: u8,
    pad1:u8,
    pad2:u8,
    pad3:u8,
    pad4:u32,
}

#[allow(unused_imports)]
use test::{Bencher, black_box};

#[bench]
fn bench_serde_serialize(b: &mut Bencher) {
    use bincode::{serialize, deserialize};
    

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

        let encoded: Vec<u8> = serialize(&test).unwrap();

        let encoded = black_box(encoded);

        let r: Vec<BenchStruct> = deserialize(&encoded[..]).unwrap();

        assert!(r.len()==1000);  
    });
}

#[bench]
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

#[test]
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

#[derive(Debug, PartialEq, Savefile)]
struct SmallStruct {
    x1: u32,
    x2: i32,
}

#[test]
pub fn test_small_struct() {
    assert_roundtrip(SmallStruct { x1: 123, x2: 321 });
}

#[derive(Debug, PartialEq, Savefile)]
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
            Serializer::save(&mut bufw, version_number1, &sample_v1).unwrap();
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    let roundtrip_result = Deserializer::load::<E2>(&mut f, version_number2).unwrap();    
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

#[derive(Debug, PartialEq, Savefile)]
struct SmallStructRem1 {
    x1: u32,
    x2: i32,
    x3: String,
}
#[derive(Debug, PartialEq, Savefile )]
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
    });;   
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
    #[versions="0..0"]
    rem : Removed<u32>,
}

#[test]
pub fn test_struct_only_removed_fields() {
    assert_roundtrip_version(OnlyRemoved{rem: Removed::new()},1);
}

