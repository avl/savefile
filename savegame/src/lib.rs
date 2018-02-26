#![recursion_limit="256"]
#![feature(test)]
#![feature(specialization)]
#![feature(attr_literals)]

extern crate byteorder;
use std::io::Write;
use std::io::Read;
use std::fmt::Debug;
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};
use std::collections::{HashMap};
use std::hash::Hash;
extern crate test;

#[macro_use]
extern crate diskstore_derive;


pub struct Serializer<'a> {
	writer: &'a mut Write,
	pub version: u32,
}

pub struct Deserializer<'a> {
	reader: &'a mut Read,
	pub file_version: u32,
	pub memory_version: u32,
}

pub unsafe trait ReprC : Copy {}

impl<'a> Serializer<'a> {
	pub fn write_u8(&mut self, v : u8) {
		self.writer.write(&[v]).unwrap();
	}
	pub fn write_i8(&mut self, v : i8) {
		self.writer.write_i8(v).unwrap();
	}
	
	pub fn write_u16(&mut self, v : u16) {
		self.writer.write_u16::<LittleEndian>(v).unwrap();
	}
	pub fn write_i16(&mut self, v : i16) {
		self.writer.write_i16::<LittleEndian>(v).unwrap();
	}
	
	pub fn write_u32(&mut self, v : u32) {
		self.writer.write_u32::<LittleEndian>(v).unwrap();
	}
	pub fn write_i32(&mut self, v : i32) {
		self.writer.write_i32::<LittleEndian>(v).unwrap();
	}
	
	pub fn write_u64(&mut self, v : u64) {
		self.writer.write_u64::<LittleEndian>(v).unwrap();
	}
	pub fn write_i64(&mut self, v : i64) {
		self.writer.write_i64::<LittleEndian>(v).unwrap();
	}
	
	pub fn write_usize(&mut self, v : usize) {
		self.writer.write_u64::<LittleEndian>(v as u64).unwrap();
	}
	pub fn write_isize(&mut self, v : isize) {
		self.writer.write_i64::<LittleEndian>(v as i64).unwrap();
	}
	pub fn write_buf(&mut self,v:&[u8]) {
		self.writer.write_all(v);
	}
	pub fn write_string(&mut self, v: &str) {
		let asb=v.as_bytes();
		self.write_usize(asb.len());
		self.writer.write_all(asb).unwrap();
	}
	
	pub fn new<'b>(writer:&'b mut Write,version:u32) -> Serializer<'b> {
		writer.write_u32::<LittleEndian>(version);
		Serializer {
			writer: writer,
			version:version
		}
	}
}

impl<'a> Deserializer<'a> {
	pub fn read_u8(&mut self) -> u8 {
		let mut buf=[0u8];
		self.reader.read_exact(&mut buf).unwrap();
		buf[0]
	}
	pub fn read_u16(&mut self) -> u16 {
		self.reader.read_u16::<LittleEndian>().unwrap()
	}
	pub fn read_u32(&mut self) -> u32 {
		self.reader.read_u32::<LittleEndian>().unwrap()
	}
	pub fn read_u64(&mut self) -> u64 {
		self.reader.read_u64::<LittleEndian>().unwrap()
	}

	pub fn read_i8(&mut self) -> i8 {
		self.reader.read_i8().unwrap()
	}
	pub fn read_i16(&mut self) -> i16 {
		self.reader.read_i16::<LittleEndian>().unwrap()
	}
	pub fn read_i32(&mut self) -> i32 {
		self.reader.read_i32::<LittleEndian>().unwrap()
	}
	pub fn read_i64(&mut self) -> i64 {
		self.reader.read_i64::<LittleEndian>().unwrap()
	}
	pub fn read_isize(&mut self) -> isize {
		self.reader.read_i64::<LittleEndian>().unwrap() as isize
	}
	pub fn read_usize(&mut self) -> usize {
		self.reader.read_u64::<LittleEndian>().unwrap() as usize
	}
	pub fn read_string(&mut self) -> String {
		let l=self.read_usize();
		let mut v=Vec::with_capacity(l);
		v.resize(l,0); //TODO: Optimize this
		self.reader.read_exact(&mut v).unwrap();
		String::from_utf8(v).unwrap()
	}
	pub fn new<'b>(reader:&'b mut Read, version:u32) -> Deserializer<'b> {
		let file_ver=reader.read_u32::<LittleEndian>().unwrap();
		if file_ver>version {
			panic!("File has later version ({}) than structs in memory ({}).",file_ver,version);
		}
		Deserializer {
			reader : reader,
			file_version : file_ver,
			memory_version : version,
		}
	}
}

pub trait Serialize {
    fn serialize(&self, serializer: &mut Serializer); //TODO: Do error handling
}

pub trait Deserialize {
    fn deserialize(deserializer: &mut Deserializer) -> Self; //TODO: Do error handling
}

impl Serialize for String {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_string(self)    
    }	
}

impl Deserialize for String {
    fn deserialize(deserializer: &mut Deserializer) -> String {
    	deserializer.read_string()
    }	
}

impl<K:Serialize+Eq+Hash,V:Serialize> Serialize for HashMap<K,V> {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_usize(self.len());
    	for (k,v) in self.iter() {
    		k.serialize(serializer);
    		v.serialize(serializer);
    	}
    }	
}

impl<K:Deserialize+Eq+Hash,V:Deserialize> Deserialize for HashMap<K,V> {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	let l=deserializer.read_usize();
    	let mut ret = HashMap::with_capacity(l);
    	for _ in 0..l {
    		ret.insert(
    			K::deserialize(deserializer),
    			V::deserialize(deserializer));
    	}
    	ret
    }	
}

pub struct Removed<T> {		
	phantom : std::marker::PhantomData<T>
}

impl<T> Removed<T> {
	pub fn new() -> Removed<T> {
		Removed {
			phantom : std::marker::PhantomData

		}

	}
}

impl<T:Serialize> Serialize for Vec<T> {
    default fn serialize(&self, serializer: &mut Serializer) {
    	let l=self.len();
    	serializer.write_usize(l);
    	for item in self.iter() {
    		item.serialize(serializer)
    	}
    }	
}


impl<T:Serialize+ReprC> Serialize for Vec<T> {
    fn serialize(&self, serializer: &mut Serializer) {
    	let l=self.len();
    	serializer.write_usize(l);
    	unsafe{
    		serializer.write_buf(
    			std::slice::from_raw_parts(
    				self.as_ptr() as *const u8,
    				std::mem::size_of::<T>()*l));
    	}
    }	
}

impl<T:Deserialize> Deserialize for Vec<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	let l=deserializer.read_usize();
    	let mut ret = Vec::with_capacity(l);
    	for _ in 0..l {
    		ret.push(T::deserialize(deserializer));
    	}
    	ret
    }	
}

impl Serialize for u8 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_u8(*self);
    }
}
impl Deserialize for u8 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_u8()
    }
}
impl Serialize for i8 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_i8(*self);
    }
}
impl Deserialize for i8 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_i8()
    }
}

impl Serialize for u16 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_u16(*self);
    }
}
impl Deserialize for u16 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_u16()
    }
}
impl Serialize for i16 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_i16(*self);
    }
}
impl Deserialize for i16 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_i16()
    }
}

impl Serialize for u32 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_u32(*self);
    }
}
impl Deserialize for u32 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_u32()
    }
}
impl Serialize for i32 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_i32(*self);
    }
}
impl Deserialize for i32 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_i32()
    }
}

impl Serialize for u64 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_u64(*self);
    }
}
impl Deserialize for u64 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_u64()
    }
}
impl Serialize for i64 {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_i64(*self);
    }
}
impl Deserialize for i64 {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_i64()
    }
}

impl Serialize for usize {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_usize(*self);
    }
}
impl Deserialize for usize {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_usize()
    }
}
impl Serialize for isize {
    fn serialize(&self, serializer: &mut Serializer) {
    	serializer.write_isize(*self);
    }
}
impl Deserialize for isize {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
    	deserializer.read_isize()
    }
}



use diskstore_derive::*;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct NonCopy {
	ncfield:u8
}


use std::io::{Cursor, Seek, SeekFrom};
use std::io::BufWriter;

pub fn assert_roundtrip<E:Serialize+Deserialize+Debug+PartialEq>(sample:E) {
    let mut f = Cursor::new(Vec::new());
    {
    	let mut bufw=BufWriter::new(&mut f);
    	{
	        let mut serializer = Serializer::new(&mut bufw,0);
	        sample.serialize(&mut serializer);     
    	}
    	bufw.flush();
        println!("Serialized data: {:?}",bufw);
    }
	f.set_position(0);
    let mut deserializer = Deserializer::new(&mut f,0);
    let roundtrip_result=E::deserialize(&mut deserializer);		
    assert_eq!(sample,roundtrip_result);
    println!("Roundtrip result: {:?}",roundtrip_result);
}


#[test]
pub fn test_struct_enum() {

	#[derive(Debug, Serialize, Deserialize, PartialEq )]
	pub enum TestStructEnum {
		Variant1{a:u8,b:u8},
		Variant2{a:u8}
	}
	assert_roundtrip(TestStructEnum::Variant1 { a: 42, b: 45 });
	assert_roundtrip(TestStructEnum::Variant2 { a: 47 });
}

#[test]
pub fn test_tuple_enum() {

	#[derive(Debug, Serialize, Deserialize, PartialEq )]
	pub enum TestTupleEnum {
		Variant1(u8)
	}
	assert_roundtrip(TestTupleEnum::Variant1(37));
}

#[test]
pub fn test_unit_enum() {

	#[derive(Debug, Serialize, Deserialize, PartialEq )]
	pub enum TestUnitEnum {
		Variant1,
		Variant2
	}
	assert_roundtrip(TestUnitEnum::Variant1);
	assert_roundtrip(TestUnitEnum::Variant2);
}

#[derive(Debug, Serialize, Deserialize, PartialEq )]
pub struct TestStruct {
	x1 : u8,
	x2 : u16,
	x3 : u32,
	x4 : u64,
	x5 : usize,
	x6 : i8,
	x7 : i16,
	x8 : i32,
	x9 : i64,
	x10 : isize,
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
	let mut v=Vec::new();
	v.push(43u8);

	assert_roundtrip(v);

}

#[test]
pub fn test_vec_of_string() {
	let mut v=Vec::new();
	v.push("hejsan".to_string());

	assert_roundtrip(v);

}


#[test]
pub fn test_hashmap() {
	let mut v=HashMap::new();
	v.insert(43,45);
	v.insert(47,49);

	assert_roundtrip(v);

}


#[test]
pub fn test_string() {
	assert_roundtrip("".to_string());
	assert_roundtrip("test string".to_string());
}



#[derive(Clone,Copy,Debug, Serialize, Deserialize, PartialEq )]
pub struct BenchStruct{
	x:usize,
	y:usize,
	z:u8
}
//unsafe impl ReprC for BenchStruct {} 

use test::{Bencher, black_box};

#[bench]
fn bench_serialize(b: &mut Bencher) {

    let mut f = Cursor::new(Vec::with_capacity(100));
	let mut bufw=BufWriter::new(&mut f);
	let mut serializer = Serializer::new(&mut bufw,0);

    let mut test=Vec::new();
    for i in 0..1000 {
    	test.push(BenchStruct {
    		x:i,
    		y:i,
    		z:0
    	})
    }
 	b.iter(|| {
 		test.serialize(&mut serializer);
    });
}

#[derive(Debug,PartialEq,Serialize,Deserialize)]
struct SmallStruct {
	x1: u32,
	x2: i32
}

#[test]
pub fn test_small_struct() {
	assert_roundtrip(SmallStruct {x1:123,x2:321});
}

#[derive(Debug,PartialEq,Serialize,Deserialize)]
struct SmallStruct2 {
	x1: u32,
	x2: i32,
	#[default_val="100"]
	#[versions="1..1000"]
	x3: String,
	#[default_val="123"]
	#[versions="1.."]
	x4: u64,
}





pub fn assert_roundtrip_to_new_version<E1:Serialize+Deserialize+Debug+PartialEq,E2:Serialize+Deserialize+Debug+PartialEq>(
	sample_v1:E1,
	version_number1:u32,
	expected_v2:E2,
	version_number2:u32) {
    let mut f = Cursor::new(Vec::new());
    {
    	let mut bufw=BufWriter::new(&mut f);
    	{
	        let mut serializer = Serializer::new(&mut bufw, version_number1);
	        sample_v1.serialize(&mut serializer);     
    	}
    	bufw.flush();
        println!("Serialized data: {:?}",bufw);
    }
	f.set_position(0);
    let mut deserializer = Deserializer::new(&mut f, version_number2);
    let roundtrip_result=E2::deserialize(&mut deserializer);		
    println!("Roundtrip result: {:?}",roundtrip_result);
    assert_eq!(expected_v2,roundtrip_result);
}


#[test]
pub fn test_small_struct_upgrade() {
	assert_roundtrip_to_new_version(
		SmallStruct {x1:123,x2:321},
		0,
		SmallStruct2 {x1:123, x2:321, x3:"100".to_string(),x4:123},
		1,
		);
}


#[derive(Debug,PartialEq,Serialize,Deserialize)]
struct SmallStructRem1 {
	x1: u32,
	x2: i32,
	x3: String,
}

#[derive(Debug,PartialEq,Serialize,Deserialize)]
struct SmallStructRem2 {
	#[versions="..0"]	
	x1: Removed<u32>,
	x2: i32,
	x3: String,
}

#[test]
pub fn test_small_struct_remove() {
	assert_roundtrip_to_new_version(
		SmallStructRem1 {x1:123,x2:321,x3:"hello".to_string()},
		0,
		SmallStructRem2 {x1:Removed::new(), x2:321, x3:"100".to_string(),x4:123},
		1,
		);
}



