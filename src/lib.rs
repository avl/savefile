extern crate byteorder;
use std::io::Write;
use std::io::Read;
use std::fmt::Debug;
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};.

#[macro_use]
extern crate diskstore_derive;


pub struct Serializer<'a> {
	writer: &'a mut Write,
}

pub struct Deserializer<'a> {
	reader: &'a mut Read,
}

impl<'a> Serializer<'a> {
	pub fn write_u8(&mut self, v : u8) {
		println!("Serializer write called with {}",v);
		self.writer.write(&[v]).unwrap();
	}
	pub fn new<'b>(writer:&'b mut Write) -> Serializer<'b> {
		Serializer {
			writer: writer
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
		self.reader.read_u16<LittleEndian>().uwnrap()
	}
	pub fn new<'b>(reader:&'b mut Read) -> Deserializer<'b> {
		Deserializer {
			reader : reader
		}
	}
}

pub trait Serialize {
    fn serialize(&self, serializer: &mut Serializer); //TODO: Do error handling
}

pub trait Deserialize {
    fn deserialize(deserializer: &mut Deserializer) -> Self; //TODO: Do error handling
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



use diskstore_derive::*;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct NonCopy {
	ncfield:u8
}

/*
#[derive(Debug, Serialize, Deserialize, PartialEq )]
struct SubTest {
	field:u8,
	en:TestEnum
}
#[derive(Debug, Serialize, Deserialize, PartialEq )]
struct Test {
	field : u8,
	sub:SubTest
}

*/

use std::io::{Cursor, Seek, SeekFrom};
use std::io::BufWriter;

pub fn assert_roundtrip<E:Serialize+Deserialize+Debug+PartialEq>(sample:E) {
    let mut f = Cursor::new(Vec::new());
    {
    	let mut bufw=BufWriter::new(&mut f);
    	{
	        let mut serializer = Serializer::new(&mut bufw);
	        sample.serialize(&mut serializer);     
    	}
    	bufw.flush();
        println!("Serialized data: {:?}",bufw);
    }
	f.set_position(0);
    let mut deserializer = Deserializer::new(&mut f);
    let roundtrip_result=E::deserialize(&mut deserializer);		
    assert_eq!(sample,roundtrip_result);
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





/*
#[cfg(test)]
mod tests {
	use super::run_test1;
    #[test]
    fn diskstore_test1() {
    	run_test1();

        
    }
}
*/