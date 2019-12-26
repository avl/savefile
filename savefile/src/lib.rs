#![allow(incomplete_features)]
#![feature(allocator_api)]
#![recursion_limit = "256"]
#![feature(test)]
#![feature(specialization)]
#![feature(core_intrinsics)]
#![feature(integer_atomics)]
#![feature(const_generics)]


/*!
This is the documentation for `savefile`

# Introduction

Savefile is a rust library to conveniently, quickly and correctly
serialize and deserialize arbitrary rust structs and enums into
an efficient and compact binary version controlled format.

The design use case is any application that needs to save large
amounts of data to disk, and support loading files from previous
versions of that application (but not from later versions!).


# Example

Here is a small example where data about a player in a hypothetical 
computer game is saved to disk using Savefile.

```
extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;


#[derive(Savefile)]
struct Player {
    name : String,
    strength : u32,
    inventory : Vec<String>,
}

fn save_player(player:&Player) {
    save_file("save.bin", 0, player).unwrap();
}

fn load_player() -> Player {
    load_file("save.bin", 0).unwrap()
}

fn main() {
	let player = Player { name: "Steve".to_string(), strength: 42,
        inventory: vec!(
            "wallet".to_string(),
            "car keys".to_string(),
            "glasses".to_string())};

    save_player(&player);

    let reloaded_player = load_player();

    assert_eq!(reloaded_player.name,"Steve".to_string());
}

```

# Handling old versions

Let's expand the above example, by creating a 2nd version of the Player struct. Let's say
you decide that your game mechanics don't really need to track the strength of the player, but
you do wish to have a set of skills per player as well as the inventory.

Mark the struct like so:


```
extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;

const GLOBAL_VERSION:u32 = 1;
#[derive(Savefile)]
struct Player {
    name : String,
    #[savefile_versions="0..0"] //Only version 0 had this field
    strength : Removed<u32>,
    inventory : Vec<String>,
    #[savefile_versions="1.."] //Only versions 1 and later have this field
    skills : Vec<String>,
}

fn save_player(file:&'static str, player:&Player) {
	// Save current version of file.
    save_file(file, GLOBAL_VERSION, player).unwrap();
}

fn load_player(file:&'static str) -> Player {
	// The GLOBAL_VERSION means we have that version of our data structures,
	// but we can still load any older version.
    load_file(file, GLOBAL_VERSION).unwrap()
}

fn main() {
	let mut player = load_player("save.bin"); //Load from previous save
	assert_eq!("Steve",&player.name); //The name from the previous version saved will remain
	assert_eq!(0,player.skills.len()); //Skills didn't exist when this was saved
	player.skills.push("Whistling".to_string());	
	save_player("newsave.bin", &player); //The version saved here will the vec of skills
}
```


# Behind the scenes

For Savefile to be able to load and save a type T, that type must implement traits 
[savefile::WithSchema], [savefile::Serialize] and [savefile::Deserialize] . The custom derive macro Savefile derives
all of these.

You can also implement these traits manually. Manual implementation can be good for:

1: Complex types for which the Savefile custom derive function does not work. For
example, trait objects or objects containing pointers.

2: Objects for which not all fields should be serialized, or which need complex
initialization (like running arbitrary code during deserialization).

Note that the three trait implementations for a particular type must be in sync.
That is, the Serialize and Deserialize traits must follow the schema defined
by the WithSchema trait for the type.

## WithSchema

The [savefile::WithSchema] trait represents a type which knows which data layout it will have
when saved. 

## Serialize

The [savefile::Serialize] trait represents a type which knows how to write instances of itself to
a `Serializer`.

## Deserialize

The [savefile::Deserialize] trait represents a type which knows how to read instances of itself from a `Deserializer`.




# Rules for managing versions

The basic rule is that the Deserialize trait implementation must be able to deserialize data from any previous version.

The WithSchema trait implementation must be able to return the schema for any previous verison.

The Serialize trait implementation only needs to support the latest version.


# Versions and derive

The derive macro used by Savefile supports multiple versions of structs. To make this work,
you have to add attributes whenever fields are removed, added or have their types changed.

When adding or removing fields, use the #[savefile_versions] attribute.

The syntax is one of the following:

```text
#[savefile_versions = "N.."]  //A field added in version N
#[savefile_versions = "..N"]  //A field removed in version N+1. That is, it existed up to and including version N.
#[savefile_versions = "N..M"] //A field that was added in version N and removed in M+1. That is, a field which existed in versions N .. up to and including M.
```

Removed fields must keep their deserialization type. This is easiest accomplished by substituting their previous type
using the `Removed<T>` type. `Removed<T>` uses zero space in RAM, but deserializes equivalently to T (with the
result of the deserialization thrown away). 

Savefile tries to validate that the `Removed<T>` type is used correctly. This validation is based on string
matching, so it may trigger false positives for other types named Removed. Please avoid using a type with
such a name. If this becomes a problem, please file an issue on github.

Using the #[savefile_versions] tag is critically important. If this is messed up, data corruption is likely.

When a field is added, its type must implement the Default trait (unless the default_val or default_fn attributes
are used).

There also exists a savefile_default_val, a default_fn and a savefile_versions_as attribute. More about these below:

## The versions attribute

Rules for using the #[savefile_versions] attribute:

 * You must keep track of what the current version of your data is. Let's call this version N.
 * You may only save data using version N (supply this number when calling `save`)
 * When data is loaded, you must supply version N as the memory-version number to `load`. Load will
   still adapt the deserialization operation to the version of the serialized data.
 * The version number N is "global" (called GLOBAL_VERSION in the previous source example). All components of the saved data must have the same version. 
 * Whenever changes to the data are to be made, the global version number N must be increased.
 * You may add a new field to your structs, iff you also give it a #[savefile_versions = "N.."] attribute. N must be the new version of your data.
 * You may remove a field from your structs. If previously it had no #[savefile_versions] attribute, you must
   add a #[savefile_versions = "..N-1"] attribute. If it already had an attribute #[savefile_versions = "M.."], you must close
   its version interval using the current version of your data: #[savefile_versions = "M..N-1"]. Whenever a field is removed,
   its type must simply be changed to Removed<T> where T is its previous type. You may never completely remove 
   items from your structs. Doing so removes backward-compatibility with that version. This will be detected at load.
   For example, if you remove a field in version 3, you should add a #[savefile_versions="..2"] attribute.
 * You may not change the type of a field in your structs, except when using the savefile_versions_as-macro.


 
## The default_val attribute

The default_val attribute is used to provide a custom default value for
primitive types, when fields are added. 

Example:

```
# #[macro_use]
# extern crate savefile_derive;

#[derive(Savefile)]
struct SomeType {
    old_field: u32,
    #[savefile_default_val="42"]
    #[savefile_versions="1.."]
    new_field: u32
}

# fn main() {}

```

In the above example, the field `new_field` will have the value 42 when
deserializing from version 0 of the protocol. If the default_val attribute
is not used, new_field will have u32::default() instead, which is 0.

The default_val attribute only works for simple types.

## The default_fn attribute

The default_fn attribute allows constructing more complex values as defaults.

```
# #[macro_use]
# extern crate savefile_derive;

fn make_hello_pair() -> (String,String) {
    ("Hello".to_string(),"World".to_string())
}
#[derive(Savefile)]
struct SomeType {
    old_field: u32,
    #[savefile_default_fn="make_hello_pair"]
    #[savefile_versions="1.."]
    new_field: (String,String)
}
# fn main() {}

```

## The ignore attribute

The ignore attribute can be used to exclude certain fields from serialization. They still 
need to be constructed during deserialization (of course), so you need to use one of the
default-attributes to make sure the field can be constructed. If none of the  default-attributes
(described above) are used, savefile will attempt to use the Default trait. 

Here is an example, where a cached value is not to be deserialized.
In this example, the value will be 0.0 after deserialization, regardless
of the value when serializing.

```
# #[macro_use]
# extern crate savefile_derive;

#[derive(Savefile)]
struct IgnoreExample {
    a: f64,
    b: f64,
    #[ignore]
    cached_product: f64
}
# fn main() {}

```



## The savefile_versions_as attribute

The savefile_versions_as attribute can be used to support changing the type of a field.

Let's say the first version of our protocol uses the following struct:

```
# #[macro_use]
# extern crate savefile_derive;

#[derive(Savefile)]
struct Employee {
    name : String,
    phone_number : u64
}
# fn main() {}

```

After a while, we realize that a u64 is a really bad choice for datatype for a phone number,
since it can't represent a number with leading 0, and also can't represent special characters
which sometimes appear in phone numbers, like '+' or '-' etc.

So, we change the type of phone_number to String:

```
# #[macro_use]
# extern crate savefile_derive;

fn convert(phone_number:u64) -> String {
    phone_number.to_string()
}
#[derive(Savefile)]
struct Employee {
    name : String,
    #[versions_as="0..0:convert:u64"]
    #[versions="1.."]
    phone_number : String
}
# fn main() {}

```

This will cause version 0 of the protocol to be deserialized expecting a u64 for the phone number,
which will then be converted using the provided function `convert` into a String.

Note, that conversions which are supported by the From trait are done automatically, and the
function need not be specified in these cases.

Let's say we have the following struct:

```
# #[macro_use]
# extern crate savefile_derive;

#[derive(Savefile)]
struct Racecar {
    max_speed_kmh : u8,
}
# fn main() {}
```

We realize that we need to increase the range of the max_speed_kmh variable, and change it like this:

```
# #[macro_use]
# extern crate savefile_derive;

#[derive(Savefile)]
struct Racecar {
    #[savefile_versions_as="0..0:u8"]
    #[savefile_versions="1.."]
    max_speed_kmh : u16,
}
# fn main() {}
```

Note that in this case we don't need to tell Savefile how the deserialized u8 is to be converted
to an u16.



# Speeding things up

Now, let's say we want to add a list of all positions that our player have visited,
so that we can provide a instant-replay function to our game. The list can become
really long, so we want to make sure that the overhead when serializing this is
as low as possible.

Savefile has an unsafe trait [savefile::ReprC] that you can implement for a type T. This instructs
Savefile to optimize serialization of Vec<T> into being a very fast, raw memory copy.

This is dangerous. You, as implementor of the `ReprC` trait take full responsibility
that all the following rules are upheld:

 * The type T is Copy
 * The host platform is little endian. The savefile disk format uses little endian. Automatic validation of this should
 * probably be added to savefile. 
 * The type T is a struct or an enum without fields. Using it on enums with fields will probably lead to silent data corruption.
 * The type is represented in memory in an ordered, packed representation. Savefile is not
 clever enough to inspect the actual memory layout and adapt to this, so the memory representation
 has to be all the types of the struct fields in a consecutive sequence without any gaps. Note
 that the #[repr(C)] trait does not do this - it will include padding if needed for alignment
 reasons. You should not use #[repr(packed)], since that may lead to unaligned struct fields.
 Instead, you should use #[repr(C)] combined with manual padding, if necessary.
 * If the type is an enum, it must be #[repr(u8)] .

For example, don't do:
```
#[repr(C)]
struct Bad {
	f1 : u8,
	f2 : u32,
}
``` 
Since the compiler is likely to insert 3 bytes of padding after f1, to ensure that f2 is aligned to 4 bytes.

Instead, do this:

```
#[repr(C)]
struct Good {
	f1 : u8,
	pad1 :u8,
	pad2 :u8,
	pad3 :u8,
	f2 : u32,
}
```

And simpy don't use the pad1, pad2 and pad3 fields. Note, at time of writing, Savefile requires that the struct 
be free of all padding. Even padding at the end is not allowed. This means that the following does not work:

```
#[repr(C)]
struct Bad2 {
	f1 : u32,
	f2 : u8,
}
``` 
This restriction may be lifted at a later time.

Note that having a struct with bad alignment will be detected, at runtime, for debug-builds. It may not be
detected in release builds. Serializing or deserializing each [savefile::ReprC] struct at least once somewhere in your test suite
is recommended.


```
extern crate savefile;
use savefile::prelude::*;

#[macro_use]
extern crate savefile_derive;

#[derive(ReprC, Clone, Copy, Savefile)]
#[repr(C)]
struct Position {
	x : u32,
	y : u32,
}

const GLOBAL_VERSION:u32 = 2;
#[derive(Savefile)]
struct Player {
    name : String,
    #[savefile_versions="0..0"] //Only version 0 had this field
    strength : Removed<u32>,
    inventory : Vec<String>,
    #[savefile_versions="1.."] //Only versions 1 and later have this field
    skills : Vec<String>,
    #[savefile_versions="2.."] //Only versions 2 and later have this field
    history : Vec<Position>
}

fn save_player(file:&'static str, player:&Player) {
    save_file(file, GLOBAL_VERSION, player).unwrap();
}

fn load_player(file:&'static str) -> Player {
    load_file(file, GLOBAL_VERSION).unwrap()
}

fn main() {
	let mut player = load_player("newsave.bin"); //Load from previous save
	player.history.push(Position{x:1,y:1});
	player.history.push(Position{x:2,y:1});
	player.history.push(Position{x:2,y:2});
	save_player("newersave.bin", &player);
}
```

*/

#[macro_use] 
extern crate failure;

pub mod prelude;
extern crate byteorder;
extern crate alloc;
extern crate arrayvec;
extern crate smallvec;
extern crate parking_lot;
use parking_lot::Mutex;
use parking_lot::RwLock;
use std::io::{Write, Error, ErrorKind};
use std::io::Read;
use std::fs::File;
use std::sync::atomic::{
    Ordering,
    AtomicBool,
    AtomicU8,AtomicI8,
    AtomicU16,AtomicI16,
    AtomicU32,AtomicI32,
    AtomicU64,AtomicI64,
    AtomicUsize,AtomicIsize,
};

use self::byteorder::{LittleEndian};
use std::mem::MaybeUninit;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::BinaryHeap;
use std::hash::Hash;
extern crate indexmap;
use indexmap::IndexMap;
use indexmap::IndexSet;
extern crate test;
extern crate bit_vec;
//use self::bit_vec::BitVec;


/// This object represents an error in deserializing or serializing
/// an item.
#[derive(Debug, Fail)]
#[must_use]
pub enum SavefileError {
    #[fail(display = "Incompatible schema detected: {}", message)]
    IncompatibleSchema {
        message: String,
    },
    #[fail(display = "IO Error: {}",io_error)]
    IOError{io_error:std::io::Error},
    #[fail(display = "Invalid utf8 character {}",msg)]
    InvalidUtf8{msg:String},
    #[fail(display = "Out of memory: {}",err)]
    OutOfMemory{err:std::alloc::AllocErr},
    #[fail(display = "Memory allocation failed because memory layout could not be specified.")]
    MemoryAllocationLayoutError,
    #[fail(display = "Arrayvec: {}",msg)]
    ArrayvecCapacityError{msg:String},
    #[fail(display = "ShortRead")]
    ShortRead,
    #[fail(display = "CryptographyError")]
    CryptographyError,
}



/// Object to which serialized data is to be written.
/// This is basically just a wrapped `std::io::Write` object
/// and a file protocol version number.
pub struct Serializer<'a> {
    writer: &'a mut dyn Write,
    pub version: u32,
}

/// Object from which bytes to be deserialized are read.
/// This is basically just a wrapped `std::io::Read` object,
/// the version number of the file being read, and the
/// current version number of the data structures in memory.
pub struct Deserializer<'a> {
    reader: &'a mut dyn Read,
    pub file_version: u32,
    pub memory_version: u32,
}




/// This is a marker trait for types which have an in-memory layout that is packed
/// and therefore identical to the layout that savefile will use on disk.
/// This means that types for which this trait is implemented can be serialized
/// very quickly by just writing their raw bits to disc.
///
/// Rules to implement this trait:
///
/// * The type must be copy
/// * The type must not contain any padding
/// * The type must have a strictly deterministic memory layout (no field order randomization). This typically means repr(C)
/// * All the constituent types of the type must also implement `ReprC` (correctly).
pub unsafe trait ReprC: Copy {
    /// This method returns true if the optimization is allowed
    /// for the protocol version given as an argument.
    /// This may return true if and only if the given protocol version
    /// has a serialized format identical to the given protocol version.
    fn repr_c_optimization_safe(version: u32) -> bool;
}

impl From<std::io::Error> for SavefileError {
    fn from(s: std::io::Error) -> SavefileError {
        SavefileError::IOError{io_error:s}
    }
}

impl From<std::alloc::AllocErr> for SavefileError {
    fn from(s: std::alloc::AllocErr) -> SavefileError {
        SavefileError::OutOfMemory{err:s}
    }
}

impl From<std::string::FromUtf8Error> for SavefileError {
    fn from(s: std::string::FromUtf8Error) -> SavefileError {
        SavefileError::InvalidUtf8{msg:s.to_string()}
    }
}

impl<T> From<arrayvec::CapacityError<T> > for SavefileError {
    fn from(s: arrayvec::CapacityError<T>) -> SavefileError {
        SavefileError::ArrayvecCapacityError{msg:s.to_string()}
    }
}




use ring::{aead};
use ring::aead::{Nonce, UnboundKey, AES_256_GCM, SealingKey, OpeningKey, NonceSequence, BoundKey};
use ring::error::Unspecified;
extern crate rand;

use rand::rngs::{OsRng};
use rand::RngCore;
use byteorder::WriteBytesExt;
use byteorder::ReadBytesExt;

extern crate ring;

#[derive(Debug)]
struct RandomNonceSequence {
    data1:u64,
    data2:u32,
}
impl RandomNonceSequence {
    pub fn new() -> RandomNonceSequence {

        RandomNonceSequence {
            data1: OsRng.next_u64(),
            data2: OsRng.next_u32(),
        }
    }
    pub fn serialize(&self, writer:&mut dyn Write) -> Result<(),SavefileError>{
        writer.write_u64::<LittleEndian>(self.data1)?;
        writer.write_u32::<LittleEndian>(self.data2)?;
        Ok(())
    }
    pub fn deserialize(reader: &mut dyn Read) -> Result<RandomNonceSequence,SavefileError> {
        Ok(RandomNonceSequence {
            data1: reader.read_u64::<LittleEndian>()?,
            data2: reader.read_u32::<LittleEndian>()?,
        })
    }
}

impl NonceSequence for RandomNonceSequence {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        self.data2 = self.data2.wrapping_add(1);
        if self.data2 == 0 {
            self.data1 = self.data1.wrapping_add(1);
        }
        use std::mem::transmute;
        let mut bytes = [0u8;12];
        let bytes1:[u8;8] = unsafe { transmute(self.data1.to_le()) };
        let bytes2:[u8;4] = unsafe { transmute(self.data2.to_le()) };
        for i in 0..8 {
            bytes[i]=bytes1[i];
        }
        for i in 0..4 {
            bytes[i+8]=bytes2[i];
        }

        Ok(Nonce::assume_unique_for_key(bytes))
    }
}


pub struct CryptoWriter<'a> {
    writer : &'a mut dyn Write,
    buf: Vec<u8>,
    sealkey : SealingKey<RandomNonceSequence>,
    failed: bool
}
pub struct CryptoReader<'a> {
    reader : &'a mut dyn Read,
    buf: Vec<u8>,
    offset:usize,
    openingkey : OpeningKey<RandomNonceSequence>
}

impl<'a> CryptoReader<'a> {
    pub fn new(reader:&'a mut dyn Read, key_bytes: [u8;32]) -> Result<CryptoReader<'a>,SavefileError> {

        let unboundkey = UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap();

        let nonce_sequence = RandomNonceSequence::deserialize(reader)?;
        let openingkey = OpeningKey::new(unboundkey, nonce_sequence);

        Ok(CryptoReader {
            reader,
            offset:0,
            buf:Vec::new(),
            openingkey
        })
    }

}

const CRYPTO_BUFSIZE : usize = 100_000;

impl<'a> Drop for CryptoWriter<'a> {
    fn drop(&mut self) {
        self.flush().expect("The implicit flush in the Drop of CryptoWriter failed. This causes this panic. If you want to be able to handle this, make sure to call flush() manually. If a manual flush has failed, Drop won't panic.");
    }
}
impl<'a> CryptoWriter<'a> {
    pub fn new(writer:&'a mut dyn Write, key_bytes: [u8;32]) -> Result<CryptoWriter<'a>,SavefileError> {
        let unboundkey = UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap();
        let nonce_sequence = RandomNonceSequence::new();
        nonce_sequence.serialize(writer)?;
        let sealkey = SealingKey::new(unboundkey, nonce_sequence);
        Ok(CryptoWriter {
            writer,
            buf: Vec::new(),
            sealkey,
            failed: false
        })
    }
    pub fn flush_final(mut self) -> Result<(),SavefileError> {
        if self.failed {
            panic!("Call to failed CryptoWriter");
        }
        self.flush()?;
        Ok(())
    }
}
impl<'a> Read for CryptoReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {

        loop {
            if buf.len() <= self.buf.len() - self.offset {
                buf.clone_from_slice(&self.buf[self.offset .. self.offset+buf.len()]);
                self.offset += buf.len();
                return Ok(buf.len())
            }

            {
                let oldlen = self.buf.len();
                let newlen = self.buf.len() - self.offset;
                self.buf.copy_within(self.offset..oldlen,0);
                self.buf.resize(newlen,0);
                self.offset = 0;

            }

            let curlen = self.reader.read_u64::<LittleEndian>()? as usize;
            if curlen > CRYPTO_BUFSIZE + 16 {
                return Err(Error::new(ErrorKind::Other,"Cryptography error"));
            }
            let orglen = self.buf.len();
            self.buf.resize(orglen + curlen,0);

            self.reader.read_exact(&mut self.buf[orglen..orglen+curlen])?;

            match self.openingkey.open_in_place(aead::Aad::empty(),
                &mut self.buf[orglen..]) {
                Ok(_) => {},
                Err(_) => { return Err(Error::new(ErrorKind::Other,"Cryptography error")); }
            }
            self.buf.resize(self.buf.len()-16,0);

        }
    }
}
impl<'a> Write for CryptoWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        if self.failed {
            panic!("Call to failed CryptoWriter");
        }
        self.buf.extend(buf);
        if self.buf.len() > CRYPTO_BUFSIZE {
            self.flush()?;
        }
        Ok(buf.len())
    }

    /// Writes any non-written buffered bytes to the underlying stream.
    /// If this fails, there is no recovery. The buffered data will have been
    /// lost. The
    fn flush(&mut self) -> Result<(), Error> {
        self.failed = true;
        let mut offset=0;

        let mut tempbuf= Vec::new();
        if self.buf.len() > CRYPTO_BUFSIZE {
            tempbuf = Vec::<u8>::with_capacity(CRYPTO_BUFSIZE+16);
        }

        while self.buf.len() > offset {

            let curbuf;
            if offset==0 && self.buf.len() <= CRYPTO_BUFSIZE {
                curbuf=&mut self.buf;
            } else {
                let chunksize = (self.buf.len() - offset).min(CRYPTO_BUFSIZE);
                tempbuf.resize(chunksize,0u8);
                tempbuf.clone_from_slice(&self.buf[offset..offset+chunksize]);
                curbuf = &mut tempbuf;
            }
            let expected_final_len = curbuf.len() as u64 + 16;
            debug_assert!(expected_final_len <= CRYPTO_BUFSIZE as u64 + 16);
            self.writer.write_u64::<LittleEndian>(expected_final_len)?; //16 for the tag
            match self.sealkey.seal_in_place_append_tag(
                aead::Aad::empty(),
                curbuf,
            ) {
                Ok(_) => {},
                Err(_) => { return Err(Error::new(ErrorKind::Other,"Cryptography error"));}
            }
            debug_assert!(curbuf.len() == expected_final_len as usize, "The size of the TAG generated by the AES 256 GCM in ring seems to have changed! This is very unexpected. File a bug on the savefile-crate");
            self.writer.write(&curbuf[..])?;
            offset += curbuf.len()-16;
        }
        self.buf.clear();
        self.failed = false;
        Ok(())
    }
}


impl<'a> Serializer<'a> {
    pub fn write_bool(&mut self, v: bool)  -> Result<(),SavefileError> {
        Ok(self.writer.write_u8( if v {1} else {0})?)
    }
    pub fn write_u8(&mut self, v: u8)  -> Result<(),SavefileError> {
        Ok(self.writer.write_all(&[v])?)
    }
    pub fn write_i8(&mut self, v: i8) -> Result<(),SavefileError> {
        Ok(self.writer.write_i8(v)?)
    }

    pub fn write_u16(&mut self, v: u16) -> Result<(),SavefileError> {
        Ok(self.writer.write_u16::<LittleEndian>(v)?)
    }
    pub fn write_i16(&mut self, v: i16) -> Result<(),SavefileError> {
        Ok(self.writer.write_i16::<LittleEndian>(v)?)
    }

    pub fn write_u32(&mut self, v: u32) -> Result<(),SavefileError> {
        Ok(self.writer.write_u32::<LittleEndian>(v)?)
    }
    pub fn write_i32(&mut self, v: i32) -> Result<(),SavefileError> {
        Ok(self.writer.write_i32::<LittleEndian>(v)?)
    }

    pub fn write_f32(&mut self, v: f32) -> Result<(),SavefileError> {
        Ok(self.writer.write_f32::<LittleEndian>(v)?)
    }
    pub fn write_f64(&mut self, v: f64) -> Result<(),SavefileError> {
        Ok(self.writer.write_f64::<LittleEndian>(v)?)
    }

    pub fn write_u64(&mut self, v: u64) -> Result<(),SavefileError> {
        Ok(self.writer.write_u64::<LittleEndian>(v)?)
    }
    pub fn write_i64(&mut self, v: i64) -> Result<(),SavefileError> {
        Ok(self.writer.write_i64::<LittleEndian>(v)?)
    }

    pub fn write_usize(&mut self, v: usize) -> Result<(),SavefileError> {
        Ok(self.writer.write_u64::<LittleEndian>(v as u64)?)
    }
    pub fn write_isize(&mut self, v: isize) -> Result<(),SavefileError> {
        Ok(self.writer.write_i64::<LittleEndian>(v as i64)?)
    }
    pub fn write_buf(&mut self, v: &[u8]) -> Result<(),SavefileError> {
        Ok(self.writer.write_all(v)?)
    }
    pub fn write_string(&mut self, v: &str) -> Result<(),SavefileError> {
        let asb = v.as_bytes();
        self.write_usize(asb.len())?;
        Ok(self.writer.write_all(asb)?)
    }
    pub fn write_bytes(&mut self, v: &[u8]) -> Result<(),SavefileError> {
        Ok(self.writer.write_all(v)?)
    }

    /// Creata a new serializer.
    /// Don't use this function directly, use the [savefile::save] function instead.
    pub fn save<T:WithSchema + Serialize>(writer: &mut dyn Write, version: u32, data: &T) -> Result<(),SavefileError> {
        Ok(Self::save_impl(writer,version,data,true)?)
    }
    /// Creata a new serializer.
    /// Don't use this function directly, use the [savefile::save_noschema] function instead.
    pub fn save_noschema<T:WithSchema + Serialize>(writer: &mut dyn Write, version: u32, data: &T) -> Result<(),SavefileError> {
        Ok(Self::save_impl(writer,version,data,false)?)
    }
    fn save_impl<T:WithSchema + Serialize>(writer: &mut dyn Write, version: u32, data: &T, with_schema: bool) -> Result<(),SavefileError> {
        writer.write_u32::<LittleEndian>(version).unwrap();

        if with_schema
        {
            let schema = T::schema(version);
            let mut schema_serializer=Serializer::new_raw(writer);
            schema.serialize(&mut schema_serializer)?;            
        }

        let mut serializer=Serializer { writer, version };
        Ok(data.serialize(&mut serializer)?)
    }

    /// Create a Serializer.
    /// Don't use this method directly, use the [savefile::save] function
    /// instead.
    pub fn new_raw(writer: &mut dyn Write) -> Serializer {
        Serializer { writer, version:0 }
    }
}

impl<'a> Deserializer<'a> {
    pub fn read_bool(&mut self) -> Result<bool,SavefileError> {
        Ok(self.reader.read_u8()? == 1)
    }
    pub fn read_u8(&mut self) -> Result<u8,SavefileError> {
        let mut buf = [0u8];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
    pub fn read_u16(&mut self) -> Result<u16,SavefileError> {
        Ok(self.reader.read_u16::<LittleEndian>()?)
    }
    pub fn read_u32(&mut self) -> Result<u32,SavefileError> {
        Ok(self.reader.read_u32::<LittleEndian>()?)
    }
    pub fn read_u64(&mut self) -> Result<u64,SavefileError> {
        Ok(self.reader.read_u64::<LittleEndian>()?)
    }

    pub fn read_i8(&mut self) -> Result<i8,SavefileError> {
        Ok(self.reader.read_i8()?)
    }
    pub fn read_i16(&mut self) -> Result<i16,SavefileError> {
        Ok(self.reader.read_i16::<LittleEndian>()?)
    }
    pub fn read_i32(&mut self) -> Result<i32,SavefileError> {
        Ok(self.reader.read_i32::<LittleEndian>()?)
    }
    pub fn read_i64(&mut self) -> Result<i64,SavefileError> {
        Ok(self.reader.read_i64::<LittleEndian>()?)
    }
    pub fn read_f32(&mut self) -> Result<f32,SavefileError> {
        Ok(self.reader.read_f32::<LittleEndian>()?)
    }
    pub fn read_f64(&mut self) -> Result<f64,SavefileError> {
        Ok(self.reader.read_f64::<LittleEndian>()?)
    }
    pub fn read_isize(&mut self) -> Result<isize,SavefileError> {
        Ok(self.reader.read_i64::<LittleEndian>()? as isize)
    }
    pub fn read_usize(&mut self) -> Result<usize,SavefileError> {
        Ok(self.reader.read_u64::<LittleEndian>()? as usize)
    }
    pub fn read_string(&mut self) -> Result<String,SavefileError> {
        let l = self.read_usize()?;
        let mut v = Vec::with_capacity(l);
        v.resize(l, 0); //TODO: Optimize this
        self.reader.read_exact(&mut v)?;
        Ok(String::from_utf8(v)?)
    }
    pub fn read_bytes(&mut self, len:usize) -> Result<Vec<u8>,SavefileError> {
        let mut v = Vec::with_capacity(len);
        v.resize(len, 0); //TODO: Optimize this
        self.reader.read_exact(&mut v)?;
        Ok(v)        
    }
    pub fn read_bytes_to_buf(&mut self, _len:usize, buf:&mut [u8]) -> Result<(),SavefileError> {
        self.reader.read_exact(buf)?;
        Ok(())        
    }

    /// Deserialize an object of type T from the given reader.
    /// Don't use this method directly, use the [savefile::load] function
    /// instead.
    pub fn load<T:WithSchema+Deserialize>(reader: &mut dyn Read, version: u32) -> Result<T,SavefileError> {
        Deserializer::load_impl::<T>(reader,version,true)
    }
    /// Deserialize an object of type T from the given reader.
    /// Don't use this method directly, use the [savefile::load_noschema] function
    /// instead.
    pub fn load_noschema<T:WithSchema+Deserialize>(reader: &mut dyn Read, version: u32) -> Result<T,SavefileError> {
        Deserializer::load_impl::<T>(reader,version,false)
    }
    fn load_impl<T:WithSchema+Deserialize>(reader: &mut dyn Read, version: u32, fetch_schema: bool) -> Result<T,SavefileError> {
        let file_ver = reader.read_u32::<LittleEndian>()?;
        if file_ver > version {
            panic!(
                "File has later version ({}) than structs in memory ({}).",
                file_ver, version
            );
        }

        if fetch_schema
        {
            let mut schema_deserializer = Deserializer::new_raw(reader);
            let memory_schema = T::schema(file_ver);
            let file_schema = Schema::deserialize(&mut schema_deserializer)?;
            
            if let Some(err) = diff_schema(&memory_schema, &file_schema,".".to_string()) {
                return Err(SavefileError::IncompatibleSchema{
                    message:format!("Saved schema differs from in-memory schema for version {}. Error: {}",file_ver,
                    err)});
            }
        }
        let mut deserializer=Deserializer {
            reader,
            file_version: file_ver,
            memory_version: version,
        };
        Ok(T::deserialize(&mut deserializer)?)
    }

    /// Create a Deserializer.
    /// Don't use this method directly, use the [savefile::load] function
    /// instead.
    pub fn new_raw(reader: &mut dyn Read) -> Deserializer {
        Deserializer {
            reader,
            file_version: 0,
            memory_version: 0,
        }
    }
}


/// Deserialize an instance of type T from the given `reader` .
/// The current type of T in memory must be equal to `version`.
/// The deserializer will use the actual protocol version in the
/// file to do the deserialization.
pub fn load<T:WithSchema+Deserialize>(reader: &mut dyn Read, version: u32) -> Result<T,SavefileError> {
    Deserializer::load::<T>(reader,version)
}

/// Write the given `data` to the `writer`. 
/// The current version of data must be `version`.
pub fn save<T:WithSchema+Serialize>(writer: &mut dyn Write, version: u32, data: &T) -> Result<(),SavefileError> {
    Serializer::save::<T>(writer,version,data)
}

/// Like [savefile::load] , but used to open files saved without schema,
/// by one of the _noschema versions of the save functions.
pub fn load_noschema<T:WithSchema+Deserialize>(reader: &mut dyn Read, version: u32) -> Result<T,SavefileError> {
    Deserializer::load_noschema::<T>(reader,version)
}

/// Write the given `data` to the `writer`. 
/// The current version of data must be `version`.
/// Do this write without writing any schema to disk.
/// As long as all the serializers and deserializers
/// are correctly written, the schema is not necessary.
/// Omitting the schema saves some space in the saved file,
/// but means that any mistake in implementation of the 
/// Serialize or Deserialize traits will cause hard-to-troubleshoot
/// data corruption instead of a nice error message.
pub fn save_noschema<T:WithSchema+Serialize>(writer: &mut dyn Write, version: u32, data: &T) -> Result<(),SavefileError> {
    Serializer::save_noschema::<T>(writer,version,data)
}

/// Like [savefile::load] , except it deserializes from the given file in the filesystem.
/// This is a pure convenience function.
pub fn load_file<T:WithSchema+Deserialize>(filepath:&str, version: u32) -> Result<T,SavefileError> {
    let mut f = File::open(filepath)?;
    Deserializer::load::<T>(&mut f, version)
}

/// Like [savefile::save] , except it opens a file on the filesystem and writes
/// the data to it. This is a pure convenience function.
pub fn save_file<T:WithSchema+Serialize>(filepath:&str, version: u32, data: &T) -> Result<(),SavefileError> {
    let mut f = File::create(filepath)?;
    Serializer::save::<T>(&mut f,version,data)
}

/// Like [savefile::load_noschema] , except it deserializes from the given file in the filesystem.
/// This is a pure convenience function.
pub fn load_file_noschema<T:WithSchema+Deserialize>(filepath:&str, version: u32) -> Result<T,SavefileError> {
    let mut f = File::open(filepath)?;
    Deserializer::load_noschema::<T>(&mut f,version)
}

/// Like [savefile::save_noschema] , except it opens a file on the filesystem and writes
/// the data to it. This is a pure convenience function.
pub fn save_file_noschema<T:WithSchema+Serialize>(filepath:&str, version: u32, data: &T) -> Result<(),SavefileError> {
    let mut f = File::create(filepath)?;
    Serializer::save_noschema::<T>(&mut f,version,data)
}

/// Like [savefile::save_file], except encrypts the data with AES256, using the SHA256 hash
/// of the password as key.
pub fn save_encrypted_file<T:WithSchema+Serialize>(filepath:&str, version: u32, data: &T, password: &str) -> Result<(),SavefileError> {
    use ring::{digest};
    let actual = digest::digest(&digest::SHA256, password.as_bytes());
    let mut  key = [0u8;32];
    let password_hash = actual.as_ref();
    assert_eq!(password_hash.len(), key.len(),"A SHA256 sum must be 32 bytes");
    key.clone_from_slice(password_hash);

    let mut f = File::create(filepath)?;
    let mut writer = CryptoWriter::new(&mut f,key)?;

    Serializer::save::<T>(&mut writer,version,data)?;
    writer.flush()?;
    Ok(())
}


/// Like [savefile::load_file], except it expects the file to be an encrypted file previously stored using
/// [savefile::save_encrypted_file].
pub fn load_encrypted_file<T:WithSchema+Deserialize>(filepath:&str, version: u32, password: &str) -> Result<T,SavefileError> {
    use ring::{digest};
    let actual = digest::digest(&digest::SHA256, password.as_bytes());
    let mut  key = [0u8;32];
    let password_hash = actual.as_ref();
    assert_eq!(password_hash.len(), key.len(),"A SHA256 sum must be 32 bytes");
    key.clone_from_slice(password_hash);

    let mut f = File::open(filepath)?;
    let mut reader = CryptoReader::new(&mut f, key).unwrap();
    Deserializer::load::<T>(&mut reader, version)
}


/// This trait must be implemented by all data structures you wish to be able to save.
/// It must encode the schema for the datastructure when saved using the given version number.
/// When files are saved, the schema is encoded into the file.
/// when loading, the schema is inspected to make sure that the load will safely succeed.
/// This is only for increased safety, the file format does not in fact use the schema for any other
/// purpose, the design is schema-less at the core, the schema is just an added layer of safety (which
/// can be disabled).
pub trait WithSchema {
    /// Returns a representation of the schema used by this Serialize implementation for the given version.
    fn schema(version:u32) -> Schema;    
}

/// This trait must be implemented for all data structures you wish to be
/// able to serialize. To actually serialize data: create a [Serializer],
/// then call serialize on your data to save, giving the Serializer
/// as an argument.
///
/// The most convenient way to implement this is to use
/// `#[macro_use]
/// extern crate savefile-derive;`
///
/// and the use #[derive(Serialize)]
pub trait Serialize : WithSchema {
    /// Serialize self into the given serializer.
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError>; //TODO: Do error handling

}

/// This trait must be implemented for all data structures you wish to
/// be able to deserialize.
///
/// The most convenient way to implement this is to use
/// `#[macro_use]
/// extern crate savefile-derive;`
///
/// and the use #[derive(Deserialize)]
pub trait Deserialize : WithSchema + Sized {
    /// Deserialize and return an instance of Self from the given deserializer.
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError>;  //TODO: Do error handling
}



/// A field is serialized according to its value.
/// The name is just for diagnostics.
#[derive(Debug,PartialEq)]
pub struct Field {
    pub name : String,
    pub value : Box<Schema>
}

/// An array is serialized by serializing its items one by one,
/// without any padding.
/// The dbg_name is just for diagnostics.
#[derive(Debug,PartialEq)]
pub struct SchemaArray {
    pub item_type : Box<Schema>,
    pub count: usize,
}

impl SchemaArray {
    fn serialized_size(&self) -> Option<usize> {
        self.item_type.serialized_size().map(|x|x* self.count)
    }
}

/// A struct is serialized by serializing its fields one by one,
/// without any padding. 
/// The dbg_name is just for diagnostics.
#[derive(Debug,PartialEq)]
pub struct SchemaStruct {
    pub dbg_name : String,
    pub fields : Vec<Field>
}
fn maybe_add(a:Option<usize>,b:Option<usize>) -> Option<usize> {
    if let Some(a) = a {
        if let Some(b) = b {
            return Some(a+b);
        }
    }   
    None 
}
impl SchemaStruct {
    fn serialized_size(&self) -> Option<usize> {
        self.fields.iter().fold(Some(0usize),|prev,x| {
            maybe_add(prev,x.value.serialized_size())
        })
    }
}


/// An enum variant is serialized as its fields, one by one,
/// without any padding.
#[derive(Debug,PartialEq)]
pub struct Variant {
    pub name : String,
    pub discriminator : u8,
    pub fields : Vec<Field>
}
impl Variant {
    fn serialized_size(&self) -> Option<usize> {
        self.fields.iter().fold(Some(0usize),|prev,x| {
            maybe_add(prev,x.value.serialized_size())
        })
    }
}

/// An enum is serialized as its u8 variant discriminator
/// followed by all the field for that variant.
/// The name of each variant, as well as its order in 
/// the enum (the discriminator), is significant.
#[derive(Debug, PartialEq)]
pub struct SchemaEnum {
    pub dbg_name : String,
    pub variants : Vec<Variant>
}


fn maybe_max(a:Option<usize>,b:Option<usize>) -> Option<usize> {
    if let Some(a) = a {
        if let Some(b) = b {
            return Some(a.max(b));
        }
    }   
    None 
}
impl SchemaEnum {

    fn serialized_size(&self) -> Option<usize> {
        let discr_size = 1usize;  //Discriminator is always 1 byte
        self.variants.iter().fold(Some(discr_size),|prev,x| {
            maybe_max(prev,x.serialized_size())
        })
    }
}


/// A primitive is serialized as the little endian
/// representation of its type, except for string,
/// which is serialized as an usize length followed
/// by the string in utf8.
#[allow(non_camel_case_types)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum SchemaPrimitive {
    schema_i8,
    schema_u8,
    schema_i16,
    schema_u16,
    schema_i32,
    schema_u32,
    schema_i64,
    schema_u64,
    schema_string,
    schema_f32,
    schema_f64,
    schema_bool,
    schema_canary1,
}
impl SchemaPrimitive {
    fn name(&self) -> &'static str {
        match *self {
            SchemaPrimitive::schema_i8 => "i8",
            SchemaPrimitive::schema_u8 => "u8",
            SchemaPrimitive::schema_i16 => "i16",
            SchemaPrimitive::schema_u16 => "u16",
            SchemaPrimitive::schema_i32 => "i32",
            SchemaPrimitive::schema_u32 => "u32",
            SchemaPrimitive::schema_i64 => "i64",
            SchemaPrimitive::schema_u64 => "u64",
            SchemaPrimitive::schema_string => "String",
            SchemaPrimitive::schema_f32 => "f32",
            SchemaPrimitive::schema_f64 => "f64",
            SchemaPrimitive::schema_bool => "bool",
            SchemaPrimitive::schema_canary1 => "u32",
        }
    }
}

impl SchemaPrimitive {
    fn serialized_size(&self) -> Option<usize> {

        match *self {
            SchemaPrimitive::schema_i8 | SchemaPrimitive::schema_u8 => Some(1),
            SchemaPrimitive::schema_i16 | SchemaPrimitive::schema_u16 => Some(2),
            SchemaPrimitive::schema_i32 | SchemaPrimitive::schema_u32 => Some(4),
            SchemaPrimitive::schema_i64 | SchemaPrimitive::schema_u64 => Some(8),
            SchemaPrimitive::schema_string => None,       
            SchemaPrimitive::schema_f32 => Some(4),
            SchemaPrimitive::schema_f64 => Some(8),
            SchemaPrimitive::schema_bool => Some(1),
            SchemaPrimitive::schema_canary1 => Some(4),
        }
    }
}

fn diff_primitive(a:SchemaPrimitive,b:SchemaPrimitive, path:&str) -> Option<String> {
    if a!=b {

        return Some(format!("At location [{}]: Application protocol has datatype {}, but disk format has {}",
            path,a.name(),b.name()));
    }
    None
}


/// The schema represents the save file format
/// of your data structure. It is an AST (Abstract Syntax Tree)
/// for consisting of various types of nodes in the savefile 
/// format. Custom Serialize-implementations cannot add new types to
/// this tree, but must reuse these existing ones. 
/// See the various enum variants for more information:
#[derive(Debug,PartialEq)]
pub enum Schema {
    /// Represents a struct. Custom implementations of Serialize may use this
    /// format are encouraged to use this format. 
    Struct(SchemaStruct),
    /// Represents an enum
    Enum(SchemaEnum),
    /// Represents a primitive: Any of the various integer types (u8, i8, u16, i16 etc...), or String
    Primitive(SchemaPrimitive),
    /// A Vector of arbitrary nodes, all of the given type
    Vector(Box<Schema>),
    /// An array of N arbitrary nodes, all of the given type
    Array(SchemaArray),
    /// An Option variable instance of the given type.
    SchemaOption(Box<Schema>),
    /// Basically a dummy value, the Schema nodes themselves report this schema if queried.
    Undefined,
    /// A zero-sized type. I.e, there is no data to serialize or deserialize.
    ZeroSize,    
}

impl Schema {
    pub fn new_tuple1<T1:WithSchema>(version:u32) -> Schema {
        Schema::Struct(
            SchemaStruct {
                dbg_name: "1-Tuple".to_string(),
                fields:vec![
                    Field {name:"0".to_string(), value: Box::new(T1::schema(version))},
                ]
            })
    }

    pub fn new_tuple2<T1:WithSchema,T2:WithSchema>(version:u32) -> Schema {
        Schema::Struct(
            SchemaStruct {
                dbg_name: "2-Tuple".to_string(),
                fields:vec![
                    Field {name:"0".to_string(), value: Box::new(T1::schema(version))},
                    Field {name:"1".to_string(), value: Box::new(T2::schema(version))}
                ]
            })
    }
    pub fn new_tuple3<T1:WithSchema,T2:WithSchema,T3:WithSchema>(version:u32) -> Schema {
        Schema::Struct(
            SchemaStruct {
                dbg_name: "3-Tuple".to_string(),
                fields:vec![
                    Field {name:"0".to_string(), value: Box::new(T1::schema(version))},
                    Field {name:"1".to_string(), value: Box::new(T2::schema(version))},
                    Field {name:"2".to_string(), value: Box::new(T3::schema(version))}
                ]
            })
    }
    pub fn new_tuple4<T1:WithSchema,T2:WithSchema,T3:WithSchema,T4:WithSchema>(version:u32) -> Schema {
        Schema::Struct(
            SchemaStruct {
                dbg_name: "4-Tuple".to_string(),
                fields:vec![
                    Field {name:"0".to_string(), value: Box::new(T1::schema(version))},
                    Field {name:"1".to_string(), value: Box::new(T2::schema(version))},
                    Field {name:"2".to_string(), value: Box::new(T3::schema(version))},
                    Field {name:"3".to_string(), value: Box::new(T4::schema(version))}
                ]
            })
    }
    pub fn serialized_size(&self) -> Option<usize> {
        match *self {
            Schema::Struct(ref schema_struct) => {
                schema_struct.serialized_size()
            }
            Schema::Enum(ref schema_enum) => {
                schema_enum.serialized_size()
            }
            Schema::Primitive(ref schema_primitive) => {
                schema_primitive.serialized_size()
            }
            Schema::Vector(ref _vector) => {
                None
            }
            Schema::Array(ref array) => {
                array.serialized_size()
            }
            Schema::SchemaOption(ref _content) => {
                None
            }
            Schema::Undefined => {
                None
            }
            Schema::ZeroSize => {
                Some(0)
            }
        }
    }
}

fn diff_vector(a:&Schema,b:&Schema,path:String) -> Option<String> {
    diff_schema(a,b,
        path + "/*")
}

fn diff_array(a:&SchemaArray,b:&SchemaArray,path:String) -> Option<String> {
    if a.count!=b.count {
        return Some(format!("At location [{}]: In memory array has length {}, but disk format length {}.",
            path,a.count,b.count));
    }

    diff_schema(&a.item_type,&b.item_type,
         format!("{}/[{}]",path,a.count))
}

fn diff_option(a:&Schema,b:&Schema,path:String) -> Option<String> {
    diff_schema(a,b,
        path + "/?")
}

fn diff_enum(a:&SchemaEnum,b:&SchemaEnum, path:String)  -> Option<String> {

    let path = (path + &b.dbg_name).to_string();
    if a.variants.len()!=b.variants.len() {
        return Some(format!("At location [{}]: In memory enum has {} variants, but disk format has {} variants.",
            path,a.variants.len(),b.variants.len()));
    }
    for i in 0..a.variants.len() {
        if a.variants[i].name!=b.variants[i].name {
            return Some(format!("At location [{}]: Enum variant #{} in memory is called {}, but in disk format it is called {}",
                &path,i, a.variants[i].name,b.variants[i].name));
        }
        if a.variants[i].discriminator!=b.variants[i].discriminator {
            return Some(format!("At location [{}]: Enum variant #{} in memory has discriminator {}, but in disk format it has {}",
                &path,i,a.variants[i].discriminator,b.variants[i].discriminator));
        }
        let r=diff_fields(&a.variants[i].fields,&b.variants[i].fields,&(path.to_string()+"/"+&b.variants[i].name).to_string(),"enum",
            "","");
        if let Some(err)=r {
            return Some(err);
        }
    }
    None
}
fn diff_struct(a:&SchemaStruct,b:&SchemaStruct,path:String) -> Option<String> {
    diff_fields(&a.fields,&b.fields,&(path+"/"+&b.dbg_name).to_string(),"struct", 
        &(" (struct ".to_string()+&a.dbg_name+")"), &(" (struct ".to_string()+&b.dbg_name+")"))
}
fn diff_fields(a:&[Field],b:&[Field],path:&str, structuretype:&str,
    extra_a:&str,extra_b:&str) -> Option<String> {
    if a.len()!=b.len() {
        return Some(format!("At location [{}]: In memory {}{} has {} fields, disk format{} has {} fields.",
            path,structuretype,extra_a,a.len(),extra_b,b.len()));
    }
    for i in 0..a.len() {
        /*
        if a[i].name!=b[i].name {
            return Some(format!("At location [{}]: Field #{} in memory{} is called {}, but in disk format{} it is called {}",
                &path,i,extra_a,a[i].name,extra_b,b[i].name));
        }*/
        let r=diff_schema(&a[i].value,&b[i].value,(path.to_string()+"/"+&b[i].name).to_string());
        if let Some(err)=r {
            return Some(err);
        }
    }
    None
}

/// Return a (kind of) human-readable description of the difference
/// between the two schemas. The schema 'a' is assumed to be the current
/// schema (used in memory).
/// Returns None if both schemas are equivalent
fn diff_schema(a:&Schema, b: &Schema, path:String) -> Option<String> {
    let (atype,btype)=match *a {
        Schema::Struct(ref xa) => {
            match *b {
                Schema::Struct(ref xb) => {
                    return diff_struct(xa,xb,path)
                },
                Schema::Enum(_) => ("struct","enum"),
                Schema::Primitive(_) => ("struct","primitive"),
                Schema::Vector(_) => ("struct","vector"),
                Schema::SchemaOption(_) => ("struct","option"),
                Schema::Undefined => ("struct","undefined"),
                Schema::ZeroSize => ("struct","zerosize"),
                Schema::Array(_) => ("struct","array"),
            }
        }
        Schema::Enum(ref xa) => {
            match *b {
                Schema::Enum(ref xb) => {
                    return diff_enum(xa,xb,path)
                },
                Schema::Struct(_) => ("enum","struct"),
                Schema::Primitive(_) => ("enum","primitive"),
                Schema::Vector(_) => ("enum","vector"),
                Schema::SchemaOption(_) => ("enum","option"),
                Schema::Undefined => ("enum","undefined"),
                Schema::ZeroSize => ("enum","zerosize"),
                Schema::Array(_) => ("enum","array"),
            }
        }
        Schema::Primitive(ref xa) => {
            match *b {
                Schema::Primitive(ref xb) => {
                    return diff_primitive(*xa,*xb,&path);
                },
                Schema::Struct(_) => ("primitive","struct"),
                Schema::Enum(_) => ("primitive","enum"),
                Schema::Vector(_) => ("primitive","vector"),
                Schema::SchemaOption(_) => ("primitive","option"),
                Schema::Undefined => ("primitive","undefined"),
                Schema::ZeroSize => ("primitive","zerosize"),
                Schema::Array(_) => ("primitive","array"),

            }
        }
        Schema::SchemaOption(ref xa) => {
            match *b {
                Schema::SchemaOption(ref xb) => {
                    return diff_option(xa,xb,path);
                },
                Schema::Struct(_) => ("option","struct"),
                Schema::Enum(_) => ("option","enum"),
                Schema::Primitive(_) => ("option","primitive"),
                Schema::Vector(_) => ("option","vector"),
                Schema::Undefined => ("option","undefined"),
                Schema::ZeroSize => ("option","zerosize"),
                Schema::Array(_) => ("option","array"),
            }
        }
        Schema::Vector(ref xa) => {
            match *b {
                Schema::Vector(ref xb) => {
                    return diff_vector(xa,xb,path);
                },
                Schema::Struct(_) => ("vector","struct"),
                Schema::Enum(_) => ("vector","enum"),
                Schema::Primitive(_) => ("vector","primitive"),
                Schema::SchemaOption(_) => ("vector","option"),
                Schema::Undefined => ("vector","undefined"),
                Schema::ZeroSize => ("vector","zerosize"),
                Schema::Array(_) => ("vector","array"),
            }
        }
        Schema::Undefined => {
            return Some(format!("At location [{}]: Undefined schema encountered.",path));
        }
        Schema::ZeroSize => {
            match *b {
                Schema::ZeroSize => {
                    return None;
                },
                Schema::Vector(_) => ("zerosize","vector"),
                Schema::Struct(_) => ("zerosize","struct"),
                Schema::Enum(_) => ("zerosize","enum"),
                Schema::SchemaOption(_) => ("zerosize","option"),
                Schema::Primitive(_) => ("zerosize","primitive"),
                Schema::Undefined => ("zerosize","undefined"),
                Schema::Array(_) => ("zerosize","array"),
            }
        }
        Schema::Array(ref xa) => {
            match *b {
                Schema::Vector(_) => ("array","vector"),
                Schema::Struct(_) => ("array","struct"),
                Schema::Enum(_) => ("array","enum"),
                Schema::Primitive(_) => ("array","primitive"),
                Schema::SchemaOption(_) => ("array","option"),
                Schema::Undefined => ("array","undefined"),
                Schema::ZeroSize => ("array","zerosize"),
                Schema::Array(ref xb) => return diff_array(xa,xb,path),
            }
        }
    };
    Some(format!("At location [{}]: In memory schema: {}, file schema: {}",
        path,atype,btype))
    
}


impl WithSchema for Field {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for Field {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError>{
        serializer.write_string(&self.name)?;
        self.value.serialize(serializer)
    }
}
impl Deserialize for Field {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(Field {
            name : deserializer.read_string()?,
            value : Box::new(Schema::deserialize(deserializer)?)
        })
    }
}
impl WithSchema for Variant {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for Variant {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_string(&self.name)?;
        serializer.write_u8(self.discriminator)?;
        serializer.write_usize(self.fields.len())?;
        for field in &self.fields  {
            field.serialize(serializer)?;
        }
        Ok(())
    }    
}



impl Deserialize for Variant {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(Variant {
            name: deserializer.read_string()?,
            discriminator: deserializer.read_u8()?,
            fields : {
                let l = deserializer.read_usize()?;
                let mut ret=Vec::new();
                for _ in 0..l {
                    ret.push(
                        Field {
                            name: deserializer.read_string()?,
                            value: Box::new(Schema::deserialize(deserializer)?)
                        });
                }
                ret
            }
        })
    }
}
impl Serialize for SchemaArray {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_usize(self.count)?;
        self.item_type.serialize(serializer)?;
        Ok(())
    }
}
impl Deserialize for SchemaArray {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let count = deserializer.read_usize()?;
        let item_type = Box::new(Schema::deserialize(deserializer)?);
        Ok(
            SchemaArray {
                count,
                item_type,
            }
        )
    }
}
impl WithSchema for SchemaArray {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }
}

impl WithSchema for SchemaStruct {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for SchemaStruct {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_string(&self.dbg_name)?;
        serializer.write_usize(self.fields.len())?;
        for field in &self.fields {            
            field.serialize(serializer)?;
        }
        Ok(())
    }
}
impl Deserialize for SchemaStruct {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let dbg_name = deserializer.read_string()?;
        let l=deserializer.read_usize()?;
        Ok(SchemaStruct {
            dbg_name,
            fields: {
                let mut ret=Vec::new();
                for _ in 0..l {
                    ret.push(Field::deserialize(deserializer)?)
                }
                ret
            }        
        })
    }
}

impl WithSchema for SchemaPrimitive {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for SchemaPrimitive {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        let discr=match *self {
            SchemaPrimitive::schema_i8 => 1,
            SchemaPrimitive::schema_u8 => 2,
            SchemaPrimitive::schema_i16 => 3,
            SchemaPrimitive::schema_u16 => 4,
            SchemaPrimitive::schema_i32 => 5,
            SchemaPrimitive::schema_u32 => 6,
            SchemaPrimitive::schema_i64 => 7,
            SchemaPrimitive::schema_u64 => 8,
            SchemaPrimitive::schema_string => 9,
            SchemaPrimitive::schema_f32 => 10,
            SchemaPrimitive::schema_f64 => 11,
            SchemaPrimitive::schema_bool => 12,
            SchemaPrimitive::schema_canary1 => 13,
        };
        serializer.write_u8(discr)
    }
}
impl Deserialize for SchemaPrimitive {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let var=match deserializer.read_u8()? {
            1 => SchemaPrimitive::schema_i8,
            2 => SchemaPrimitive::schema_u8,
            3 => SchemaPrimitive::schema_i16,
            4 => SchemaPrimitive::schema_u16,
            5 => SchemaPrimitive::schema_i32,
            6 => SchemaPrimitive::schema_u32,
            7 => SchemaPrimitive::schema_i64,
            8 => SchemaPrimitive::schema_u64,
            9 => SchemaPrimitive::schema_string,
            10 => SchemaPrimitive::schema_f32,
            11 => SchemaPrimitive::schema_f64,
            12 => SchemaPrimitive::schema_bool,
            13 => SchemaPrimitive::schema_canary1,
            c => panic!("Corrupt schema, primitive type #{} encountered",c),
        };
        Ok(var)
    }
}

impl WithSchema for SchemaEnum {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}

impl Serialize for SchemaEnum {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_string(&self.dbg_name)?;
        serializer.write_usize(self.variants.len())?;
        for var in &self.variants {            
            var.serialize(serializer)?;
        }
        Ok(())
    }
}
impl Deserialize for SchemaEnum {
    fn deserialize(deserializer:&mut Deserializer) -> Result<Self,SavefileError> {
        let dbg_name = deserializer.read_string()?;
        let l = deserializer.read_usize()?;
        let mut ret=Vec::new();
        for _ in 0..l {
            ret.push(Variant::deserialize(deserializer)?);
        }
        Ok(SchemaEnum {
            dbg_name,
            variants: ret
        })
    }
}

impl WithSchema for Schema {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for Schema {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        match *self {
            Schema::Struct(ref schema_struct) => {
                serializer.write_u8(1)?;
                schema_struct.serialize(serializer)
            },
            Schema::Enum(ref schema_enum) => {
                serializer.write_u8(2)?;
                schema_enum.serialize(serializer)
            },
            Schema::Primitive(ref schema_prim) => {
                serializer.write_u8(3)?;
                schema_prim.serialize(serializer)
            },
            Schema::Vector(ref schema_vector) => {
                serializer.write_u8(4)?;
                schema_vector.serialize(serializer)
            },
            Schema::Undefined => {
                serializer.write_u8(5)
            },
            Schema::ZeroSize => {
                serializer.write_u8(6)
            },
            Schema::SchemaOption(ref content) => {
                serializer.write_u8(7)?;
                content.serialize(serializer)
            },
            Schema::Array(ref array) => {
                serializer.write_u8(8)?;
                array.serialize(serializer)
            },
        }
    }    
}


impl Deserialize for Schema {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let schema=match deserializer.read_u8()? {
            1 => Schema::Struct(SchemaStruct::deserialize(deserializer)?),
            2 => Schema::Enum(SchemaEnum::deserialize(deserializer)?),
            3 => Schema::Primitive(SchemaPrimitive::deserialize(deserializer)?),
            4 => Schema::Vector(Box::new(Schema::deserialize(deserializer)?)),
            5 => Schema::Undefined,
            6 => Schema::ZeroSize,
            7 => Schema::SchemaOption(Box::new(Schema::deserialize(deserializer)?)),
            8 => Schema::Array(SchemaArray::deserialize(deserializer)?),
            c => panic!("Corrupt schema, schema variant had value {}", c),
        };

        Ok(schema)

    }
}

impl WithSchema for String {
    fn schema(_version:u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }    
}

impl Serialize for String {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_string(self)
    }
}

impl Deserialize for String {
    fn deserialize(deserializer: &mut Deserializer) -> Result<String,SavefileError> {
        deserializer.read_string()
    }
}


impl<T:WithSchema> WithSchema for Mutex<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }    
}

impl<T:Serialize> Serialize for Mutex<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        let data = self.lock();
        data.serialize(serializer)
    }
}

impl<T:Deserialize> Deserialize for Mutex<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Mutex<T>,SavefileError> {
        Ok(Mutex::new(T::deserialize(deserializer)?))
    }
}


impl<T:WithSchema> WithSchema for RwLock<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }    
}

impl<T:Serialize> Serialize for RwLock<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        let data = self.read();
        data.serialize(serializer)
    }
}

impl<T:Deserialize> Deserialize for RwLock<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<RwLock<T>,SavefileError> {
        Ok(RwLock::new(T::deserialize(deserializer)?))
    }
}


impl<K: WithSchema + Eq + Hash, V: WithSchema, S: ::std::hash::BuildHasher> WithSchema
    for HashMap<K, V, S> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(
            Schema::Struct(SchemaStruct{
                dbg_name: "KeyValuePair".to_string(),
                fields: vec![
                    Field {
                        name: "key".to_string(),
                        value: Box::new(K::schema(version)),
                    },
                    Field {
                        name: "value".to_string(),
                        value: Box::new(V::schema(version)),
                    },
                ]
            })))
    }        
}


impl<K: Serialize + Eq + Hash, V: Serialize, S: ::std::hash::BuildHasher> Serialize
    for HashMap<K, V, S>
{
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_usize(self.len())?;
        for (k, v) in self.iter() {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}


impl<K: Deserialize + Eq + Hash, V: Deserialize> Deserialize for HashMap<K, V> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = HashMap::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}


impl<K: WithSchema + Eq + Hash, V: WithSchema, S: ::std::hash::BuildHasher> WithSchema
    for IndexMap<K, V, S> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(
            Schema::Struct(SchemaStruct{
                dbg_name: "KeyValuePair".to_string(),
                fields: vec![
                    Field {
                        name: "key".to_string(),
                        value: Box::new(K::schema(version)),
                    },
                    Field {
                        name: "value".to_string(),
                        value: Box::new(V::schema(version)),
                    },
                ]
            })))
    }        
}


impl<K: Serialize + Eq + Hash, V: Serialize, S: ::std::hash::BuildHasher> Serialize
    for IndexMap<K, V, S>
{
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_usize(self.len())?;
        for (k, v) in self.iter() {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}


impl<K: Deserialize + Eq + Hash, V: Deserialize> Deserialize for IndexMap<K, V> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = IndexMap::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

impl<K: WithSchema + Eq + Hash, S: ::std::hash::BuildHasher> WithSchema
    for IndexSet<K, S> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(
            Schema::Struct(SchemaStruct{
                dbg_name: "Key".to_string(),
                fields: vec![
                    Field {
                        name: "key".to_string(),
                        value: Box::new(K::schema(version)),
                    },
                ]
            })))
    }        
}


impl<K: Serialize + Eq + Hash, S: ::std::hash::BuildHasher> Serialize
    for IndexSet<K, S>
{
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_usize(self.len())?;
        for k in self.iter() {
            k.serialize(serializer)?;
        }
        Ok(())
    }
}


impl<K: Deserialize + Eq + Hash> Deserialize for IndexSet<K> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = IndexSet::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}


#[derive(Debug, PartialEq)]
pub struct Removed<T> {
    phantom: std::marker::PhantomData<T>,
}

impl<T> Removed<T> {
    pub fn new() -> Removed<T> {
        Removed {
            phantom: std::marker::PhantomData,
        }
    }
}
impl<T:WithSchema> WithSchema for Removed<T> {
    fn schema(version:u32) -> Schema {
        <T>::schema(version)
    }    
}


impl<T:WithSchema> Serialize for Removed<T> {
    fn serialize(&self, _serializer: &mut Serializer) -> Result<(),SavefileError> {
        panic!("Something is wrong with version-specification of fields - there was an attempt to actually serialize a removed field!");
    }
}
impl<T: WithSchema + Deserialize> Deserialize for Removed<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        T::deserialize(deserializer)?;
        Ok(Removed {
            phantom: std::marker::PhantomData,
        })
    }
}


impl<T> WithSchema for std::marker::PhantomData<T> {
    fn schema(_version:u32) -> Schema {
        Schema::ZeroSize
    }    
}


impl<T> Serialize for std::marker::PhantomData<T> {
    fn serialize(&self, _serializer: &mut Serializer) -> Result<(),SavefileError> {        
        Ok(())
    }
}
impl<T> Deserialize for std::marker::PhantomData<T> {
    fn deserialize(_deserializer: &mut Deserializer) -> Result<Self,SavefileError> {        
        Ok(std::marker::PhantomData)
    }
}



impl<T:WithSchema> WithSchema for Option<T> {fn schema(version:u32) -> Schema {Schema::SchemaOption(Box::new(T::schema(version)))}}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        match self {
            &Some(ref x) => {serializer.write_bool(true)?;x.serialize(serializer)},
            &None => serializer.write_bool(false)
        }
    }
}
impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let issome=deserializer.read_bool()?;
        if issome {
            Ok(Some(T::deserialize(deserializer)?))
        } else {
            Ok(None)
        }
    }
}

impl WithSchema for bit_vec::BitVec {
    fn schema(version:u32) -> Schema {
        Schema::Struct(SchemaStruct{
            dbg_name : "BitVec".to_string(),
            fields : vec![
                Field {
                    name: "num_bits".to_string(),
                    value: Box::new(usize::schema(version)),
                },
                Field {
                    name: "num_bytes".to_string(),
                    value: Box::new(usize::schema(version)),
                },
                Field {
                    name: "buffer".to_string(),
                    value: Box::new(Schema::Vector(
                        Box::new(u8::schema(version))
                    )),
                },
            ]
        })
    }
}

impl Serialize for bit_vec::BitVec {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        let l = self.len();
        serializer.write_usize(l)?;
        let bytes=self.to_bytes();
        serializer.write_usize(bytes.len())?;
        serializer.write_bytes(&bytes)?;
        Ok(())            
    }
}
impl Deserialize for bit_vec::BitVec {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let numbits = deserializer.read_usize()?;
        let numbytes= deserializer.read_usize()?;
        let bytes = deserializer.read_bytes(numbytes)?;
        let mut ret=bit_vec::BitVec::from_bytes(&bytes);
        ret.truncate(numbits);
        Ok(ret)
    }
}
    


impl<T: WithSchema> WithSchema for BinaryHeap<T> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}
impl<T: Serialize+Ord> Serialize for BinaryHeap<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        let l = self.len();
        serializer.write_usize(l)?;
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())            
    }
}
impl<T: Deserialize+Ord> Deserialize for BinaryHeap<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = BinaryHeap::with_capacity(l);
        for _ in 0..l {
            ret.push(T::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}
    

impl<T:smallvec::Array> WithSchema for smallvec::SmallVec<T> 
    where T::Item : WithSchema {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(T::Item::schema(version)))
    }
}

impl<T:smallvec::Array> Serialize for smallvec::SmallVec<T> 
    where T::Item : Serialize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        let l = self.len();
        serializer.write_usize(l)?;
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())
    }
}
impl<T:smallvec::Array> Deserialize for smallvec::SmallVec<T> 
    where T::Item : Deserialize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = Self::with_capacity(l);
        for _ in 0..l {
            ret.push(T::Item::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}



fn regular_serialize_vec<T: Serialize>(item: &[T], serializer: &mut Serializer) -> Result<(),SavefileError> {
    let l = item.len();
    serializer.write_usize(l)?;
    for item in item.iter() {
        item.serialize(serializer)?
    }
    Ok(())
}

impl<T: WithSchema> WithSchema for Vec<T> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    default fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        regular_serialize_vec(self, serializer)
    }
}

impl<T: Serialize + ReprC> Serialize for Vec<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        unsafe {
            if !T::repr_c_optimization_safe(serializer.version) {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}

fn regular_deserialize_vec<T: Deserialize>(deserializer: &mut Deserializer) -> Result<Vec<T>,SavefileError> {
    let l = deserializer.read_usize()?;
    let mut ret = Vec::with_capacity(l);
    for _ in 0..l {
        ret.push(T::deserialize(deserializer)?);
    }
    Ok(ret)
}

impl<T: Deserialize> Deserialize for Vec<T> {
    default fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(regular_deserialize_vec::<T>(deserializer)?)
    }
}

impl<T: Deserialize + ReprC> Deserialize for Vec<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        if !T::repr_c_optimization_safe(deserializer.file_version) {
            Ok(regular_deserialize_vec::<T>(deserializer)?)
        } else {
            use std::mem;
            
            let align = mem::align_of::<T>();
            let elem_size = mem::size_of::<T>();
            let num_elems = deserializer.read_usize()?;
            if num_elems == 0 {
                return Ok(Vec::new());
            }
            let num_bytes = elem_size * num_elems;
            let layout = if let Ok(layout) = std::alloc::Layout::from_size_align(num_bytes, align) {
                Ok(layout)
            } else {
                Err(SavefileError::MemoryAllocationLayoutError)
            }?;
            let ptr = unsafe { std::alloc::alloc(layout.clone()) };

            {
                let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, num_bytes) };
                match deserializer.reader.read_exact(slice) {
                    Ok(()) => {Ok(())}
                    Err(err) => {
                        unsafe {
                            std::alloc::dealloc(ptr, layout);
                        }
                        Err(err)
                    }
                }?;
            }
            let ret=unsafe { Vec::from_raw_parts(ptr as *mut T, num_elems, num_elems) };
            Ok(ret)
        }
    }
}


impl<T: WithSchema> WithSchema for VecDeque<T> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}

impl<T: Serialize> Serialize for VecDeque<T> {
    default fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        regular_serialize_vecdeque::<T>(self, serializer)
    }
}

impl<T: Deserialize> Deserialize for VecDeque<T> {
    default fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(regular_deserialize_vecdeque::<T>(deserializer)?)
    }    
}



fn regular_serialize_vecdeque<T: Serialize>(item: &VecDeque<T>, serializer: &mut Serializer) -> Result<(),SavefileError> {
    let l = item.len();
    serializer.write_usize(l)?;
    for item in item.iter() {
        item.serialize(serializer)?
    }
    Ok(())
}


fn regular_deserialize_vecdeque<T: Deserialize>(deserializer: &mut Deserializer) -> Result<VecDeque<T>,SavefileError> {
    let l = deserializer.read_usize()?;
    let mut ret = VecDeque::with_capacity(l);
    for _ in 0..l {
        ret.push_back(T::deserialize(deserializer)?);
    }
    Ok(ret)
}

    
unsafe impl ReprC for bool {fn repr_c_optimization_safe(_version:u32) -> bool {false}} //Hard to know if bool will always be represented by single byte. Seems to depend on a lot of stuff.
unsafe impl ReprC for u8 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for i8 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for u16 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for i16 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for u32 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for i32 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for u64 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for i64 {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for usize {fn repr_c_optimization_safe(_version:u32) -> bool {true}}
unsafe impl ReprC for isize {fn repr_c_optimization_safe(_version:u32) -> bool {true}}


impl<T: WithSchema, const N:usize> WithSchema for [T; N] {
    fn schema(version:u32) -> Schema {
        Schema::Array(SchemaArray {
            item_type: Box::new(T::schema(version)),
            count: N
        })
    }
}



impl<T: Serialize, const N:usize> Serialize for [T; N] {
    default fn serialize(&self, serializer: &mut Serializer) -> Result<(), SavefileError> {
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())
    }
}
impl<T: Deserialize, const N:usize> Deserialize for [T;N] {
    default fn deserialize(deserializer: &mut Deserializer) -> Result<Self, SavefileError> {
        let mut data: [MaybeUninit<T>; N] = unsafe {
            MaybeUninit::uninit().assume_init() //This seems strange, but is correct according to rust docs: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html
        };
        for idx in 0..N {
            data[idx] = MaybeUninit::new(T::deserialize(deserializer)?); //This leaks on panic, but we shouldn't panic and at least it isn't UB!
        }
        let ptr = &mut data as *mut _ as *mut [T; N];
        let res = unsafe { ptr.read() };
        core::mem::forget(data);
        Ok( res )
    }
}

impl<T: Serialize + ReprC, const N:usize> Serialize for [T; N] {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        unsafe {
            if !T::repr_c_optimization_safe(serializer.version) {
                for item in self.iter() {
                    item.serialize(serializer)?
                }
                Ok(())
            } else {
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<T>() * N,
                ))
            }
        }
    }
}

impl<T: Deserialize + ReprC, const N: usize> Deserialize for [T;N] {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        if !T::repr_c_optimization_safe(deserializer.file_version) {
            let mut data: [MaybeUninit<T>; N] = unsafe {
                MaybeUninit::uninit().assume_init() //This seems strange, but is correct according to rust docs: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html
            };
            for idx in 0..N {
                data[idx] = MaybeUninit::new(T::deserialize(deserializer)?); //This leaks on panic, but we shouldn't panic and at least it isn't UB!
            }
            let ptr = &mut data as *mut _ as *mut [T; N];
            let res = unsafe { ptr.read() };
            core::mem::forget(data);
            Ok( res )
        } else {
            let mut data: [MaybeUninit<T>; N] = unsafe {
                MaybeUninit::uninit().assume_init() //This seems strange, but is correct according to rust docs: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html
            };

            {
                let ptr = data.as_ptr();
                let num_bytes = std::mem::size_of::<T>() * N;
                let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, num_bytes) };
                deserializer.reader.read_exact(slice)?;
            }
            let ptr = &mut data as *mut _ as *mut [T; N];
            let res = unsafe { ptr.read() };
            core::mem::forget(data);
            Ok( res )
        }
    }
}






/*
impl<T1> WithSchema for [T1;0] {
    fn schema(_version:u32) -> Schema {
        Schema::ZeroSize
    }
}
impl<T1> Serialize for [T1;0] {
    fn serialize(&self, _serializer: &mut Serializer) -> Result<(),SavefileError> {        
        Ok(())
    }
}
impl<T1> Deserialize for [T1;0] {
    fn deserialize(_deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok([])
    }
}



impl<T1:WithSchema> WithSchema for [T1;1] {
    fn schema(version:u32) -> Schema {
        Vector::new_tuple1::<T1>(version)
    }
}
impl<T1:Serialize> Serialize for [T1;1] {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self[0].serialize(serializer)
    }
}
impl<T1:Deserialize> Deserialize for [T1;1] {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            [T1::deserialize(deserializer)?]
        )
    }
}

impl<T1:WithSchema> WithSchema for [T1;2] {
    fn schema(version:u32) -> Schema {
        Schema::new_tuple2::<T1,T1>(version)
    }
}
impl<T1:Serialize> Serialize for [T1;2] {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self[0].serialize(serializer)?;
        self[1].serialize(serializer)?;
        Ok(())
    }
}
impl<T1:Deserialize> Deserialize for [T1;2] {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            [T1::deserialize(deserializer)?,
             T1::deserialize(deserializer)?]
        )
    }
}

impl<T1:WithSchema> WithSchema for [T1;3] {
    fn schema(version:u32) -> Schema {
        Schema::new_tuple3::<T1,T1,T1>(version)
    }
}
impl<T1:Serialize> Serialize for [T1;3] {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self[0].serialize(serializer)?;
        self[1].serialize(serializer)?;
        self[2].serialize(serializer)?;
        Ok(())

    }
}
impl<T1:Deserialize> Deserialize for [T1;3] {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            [T1::deserialize(deserializer)?,
             T1::deserialize(deserializer)?,
             T1::deserialize(deserializer)?,]
        )
    }
}


impl<T1:WithSchema> WithSchema for [T1;4] {
    fn schema(version:u32) -> Schema {
        Schema::new_tuple4::<T1,T1,T1,T1>(version)
    }
}
impl<T1:Serialize> Serialize for [T1;4] {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self[0].serialize(serializer)?;
        self[1].serialize(serializer)?;
        self[2].serialize(serializer)?;
        self[3].serialize(serializer)?;
        Ok(())

    }
}
impl<T1:Deserialize> Deserialize for [T1;4] {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            [T1::deserialize(deserializer)?,
             T1::deserialize(deserializer)?,
             T1::deserialize(deserializer)?,
             T1::deserialize(deserializer)?,
             ]
        )
    }
}
*/


impl<T1:WithSchema,T2:WithSchema,T3:WithSchema> WithSchema for (T1,T2,T3) {
    fn schema(version:u32) -> Schema {
        Schema::new_tuple3::<T1,T2,T3>(version)
    }
}
impl<T1:Serialize,T2:Serialize,T3:Serialize> Serialize for (T1,T2,T3) {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self.0.serialize(serializer)?;
        self.1.serialize(serializer)?;
        self.2.serialize(serializer)
    }
}
impl<T1:Deserialize,T2:Deserialize,T3:Deserialize> Deserialize for (T1,T2,T3) {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            (T1::deserialize(deserializer)?,
             T2::deserialize(deserializer)?,
             T3::deserialize(deserializer)?
             )
        )
    }
}



impl<T1:WithSchema,T2:WithSchema> WithSchema for (T1,T2) {
    fn schema(version:u32) -> Schema {
        Schema::new_tuple2::<T1,T2>(version)
    }
}
impl<T1:Serialize,T2:Serialize> Serialize for (T1,T2) {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self.0.serialize(serializer)?;
        self.1.serialize(serializer)
    }
}
impl<T1:Deserialize,T2:Deserialize> Deserialize for (T1,T2) {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            (T1::deserialize(deserializer)?,
             T2::deserialize(deserializer)?)
        )
    }
}

impl<T1:WithSchema> WithSchema for (T1,) {
    fn schema(version:u32) -> Schema {
        Schema::new_tuple1::<T1>(version)
    }
}
impl<T1:Serialize> Serialize for (T1,) {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        self.0.serialize(serializer)
    }
}
impl<T1:Deserialize> Deserialize for (T1,) {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(
            (T1::deserialize(deserializer)?,)
        )
    }
}



impl<T:arrayvec::Array<Item = u8> > WithSchema for arrayvec::ArrayString<T> {
    fn schema(_version:u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }
}
impl<T:arrayvec::Array<Item = u8> > Serialize for arrayvec::ArrayString<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        serializer.write_string(self.as_str())
    }
}
impl<T:arrayvec::Array<Item = u8> > Deserialize for arrayvec::ArrayString<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let s = deserializer.read_string()?;
        Ok(arrayvec::ArrayString::from(&s)?)
    }
}

use std::ops::Deref;
impl<T:WithSchema> WithSchema for Box<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }
}
impl<T:Serialize> Serialize for Box<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        self.deref().serialize(serializer)
    }
}
impl<T:Deserialize> Deserialize for Box<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(Box::new(T::deserialize(deserializer)?))
    }
}

use std::rc::Rc;

impl<T:WithSchema> WithSchema for Rc<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }
}
impl<T:Serialize> Serialize for Rc<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        self.deref().serialize(serializer)
    }
}
impl<T:Deserialize> Deserialize for Rc<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(Rc::new(T::deserialize(deserializer)?))
    }
}


use std::sync::Arc;

impl<T:WithSchema> WithSchema for Arc<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }
}
impl<T:Serialize> Serialize for Arc<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        self.deref().serialize(serializer)
    }
}
impl<T:Deserialize> Deserialize for Arc<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(Arc::new(T::deserialize(deserializer)?))
    }
}

use std::cell::RefCell;
use std::cell::Cell;
use std::fmt::Debug;

impl<T:WithSchema> WithSchema for RefCell<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }
}
impl<T:Serialize> Serialize for RefCell<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        self.borrow().serialize(serializer)
    }
}
impl<T:Deserialize> Deserialize for RefCell<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(RefCell::new(T::deserialize(deserializer)?))
    }
}


impl<T:WithSchema> WithSchema for Cell<T> {
    fn schema(version:u32) -> Schema {
        T::schema(version)
    }
}
impl<T:Serialize+Copy> Serialize for Cell<T> {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {        
        let t:T = self.get();
        t.serialize(serializer)
    }
}
impl<T:Deserialize> Deserialize for Cell<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(Cell::new(T::deserialize(deserializer)?))
    }
}



impl WithSchema for () {
    fn schema(_version:u32) -> Schema {
        Schema::ZeroSize
    }
}
impl Serialize for () {
    fn serialize(&self, _serializer: &mut Serializer) -> Result<(),SavefileError> {        
        Ok(())
    }
}
impl Deserialize for () {
    fn deserialize(_deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(())
    }
}



impl WithSchema for AtomicBool {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_bool)}}
impl WithSchema for AtomicU8 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u8)}}
impl WithSchema for AtomicI8 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i8)}}
impl WithSchema for AtomicU16 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u16)}}
impl WithSchema for AtomicI16 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i16)}}
impl WithSchema for AtomicU32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u32)}}
impl WithSchema for AtomicI32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i32)}}
impl WithSchema for AtomicU64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u64)}}
impl WithSchema for AtomicI64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i64)}}
impl WithSchema for AtomicUsize {fn schema(_version:u32) -> Schema {
        match std::mem::size_of::<usize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_u32),
            8 => Schema::Primitive(SchemaPrimitive::schema_u64),
            _ => panic!("Size of usize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
}}
impl WithSchema for AtomicIsize {fn schema(_version:u32) -> Schema {
        match std::mem::size_of::<isize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_i32),
            8 => Schema::Primitive(SchemaPrimitive::schema_i64),
            _ => panic!("Size of isize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
}}


impl WithSchema for bool {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_bool)}}
impl WithSchema for u8 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u8)}}
impl WithSchema for i8 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i8)}}
impl WithSchema for u16 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u16)}}
impl WithSchema for i16 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i16)}}
impl WithSchema for u32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u32)}}
impl WithSchema for i32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i32)}}
impl WithSchema for u64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u64)}}
impl WithSchema for i64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i64)}}
impl WithSchema for usize {fn schema(_version:u32) -> Schema {
        match std::mem::size_of::<usize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_u32),
            8 => Schema::Primitive(SchemaPrimitive::schema_u64),
            _ => panic!("Size of usize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
}}
impl WithSchema for isize {fn schema(_version:u32) -> Schema {
        match std::mem::size_of::<isize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_i32),
            8 => Schema::Primitive(SchemaPrimitive::schema_i64),
            _ => panic!("Size of isize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
}}
impl WithSchema for f32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_f32)}}
impl WithSchema for f64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_f64)}}

impl Serialize for u8 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u8(*self)
    }
}
impl Deserialize for u8 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_u8()
    }
}
impl Serialize for bool {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_bool(*self)
    }
}
impl Deserialize for bool {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_bool()
    }
}



impl Serialize for f32 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_f32(*self)
    }
}
impl Deserialize for f32 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_f32()
    }
}

impl Serialize for f64 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_f64(*self)
    }
}
impl Deserialize for f64 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_f64()
    }
}

impl Serialize for i8 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i8(*self)
    }
}
impl Deserialize for i8 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_i8()
    }
}
impl Serialize for u16 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u16(*self)
    }
}
impl Deserialize for u16 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_u16()
    }
}
impl Serialize for i16 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i16(*self)
    }
}
impl Deserialize for i16 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_i16()
    }
}

impl Serialize for u32 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u32(*self)
    }
}
impl Deserialize for u32 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_u32()
    }
}
impl Serialize for i32 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i32(*self)
    }
}
impl Deserialize for i32 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_i32()
    }
}

impl Serialize for u64 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u64(*self)
    }
}
impl Deserialize for u64 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_u64()
    }
}
impl Serialize for i64 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i64(*self)
    }
}
impl Deserialize for i64 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_i64()
    }
}

impl Serialize for usize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_usize(*self)
    }
}
impl Deserialize for usize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_usize()
    }
}
impl Serialize for isize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_isize(*self)
    }
}
impl Deserialize for isize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        deserializer.read_isize()
    }
}








impl Serialize for AtomicBool {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_bool(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicBool {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicBool::new(deserializer.read_bool()?))
    }
}

impl Serialize for AtomicU8 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u8(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU8 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicU8::new(deserializer.read_u8()?))
    }
}
impl Serialize for AtomicI8 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i8(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI8 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicI8::new(deserializer.read_i8()?))
    }
}
impl Serialize for AtomicU16 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u16(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU16 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicU16::new(deserializer.read_u16()?))
    }
}
impl Serialize for AtomicI16 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i16(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI16 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicI16::new(deserializer.read_i16()?))
    }
}

impl Serialize for AtomicU32 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u32(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU32 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicU32::new(deserializer.read_u32()?))
    }
}
impl Serialize for AtomicI32 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i32(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI32 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicI32::new(deserializer.read_i32()?))
    }
}

impl Serialize for AtomicU64 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u64(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU64 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicU64::new(deserializer.read_u64()?))
    }
}
impl Serialize for AtomicI64 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_i64(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI64 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicI64::new(deserializer.read_i64()?))
    }
}

impl Serialize for AtomicUsize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_usize(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicUsize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicUsize::new(deserializer.read_usize()?))
    }
}
impl Serialize for AtomicIsize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_isize(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicIsize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        Ok(AtomicIsize::new(deserializer.read_isize()?))
    }
}

#[derive(Clone,Copy,Eq,PartialEq,Default,Debug)]
pub struct Canary1 {
}
impl Canary1 {
    pub fn new() -> Canary1 {
        Canary1 {}
    }
}
impl Deserialize for Canary1 {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self,SavefileError> {
        let magic = deserializer.read_u32()?;
        if magic != 0x47566843 {
            panic!("Encountered bad magic value when deserializing Canary1. Expected {} but got {}",
                0x47566843,magic);            
        }
        Ok(Canary1{})
    }
}

impl Serialize for Canary1 {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(),SavefileError> {
        serializer.write_u32(0x47566843)
    }
}

impl WithSchema for Canary1 {
    fn schema(_version:u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_canary1)
    }    
}

