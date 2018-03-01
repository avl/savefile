extern crate byteorder;

extern crate alloc;
use std;
use std::io::Write;
use std::io::Read;
use self::byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::hash::Hash;
extern crate test;

/// Object to which serialized data is to be written.
/// This is basically just a wrapped [std::io::Write] object
/// and a file protocol version number.
pub struct Serializer<'a> {
    writer: &'a mut Write,
    pub version: u32,
}

/// Object from which bytes to be deserialized are read.
/// This is basically just a wrapped [std::io::Read] object,
/// the version number of the file being read, and the
/// current version number of the data structures in memory.
pub struct Deserializer<'a> {
    reader: &'a mut Read,
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
/// * All the constituent types of the type must also implement ReprC (correctly).
pub unsafe trait ReprC: Copy {
    /// This method returns true if the optimization is allowed
    /// for the protocol version given as an argument.
    /// This may return true if and only if the given protocol version
    /// has a serialized format identical to the given protocol version.
    fn repr_c_optimization_safe(version: u32) -> bool;
}

impl<'a> Serializer<'a> {
    pub fn write_u8(&mut self, v: u8) {
        self.writer.write_all(&[v]).unwrap();
    }
    pub fn write_i8(&mut self, v: i8) {
        self.writer.write_i8(v).unwrap();
    }

    pub fn write_u16(&mut self, v: u16) {
        self.writer.write_u16::<LittleEndian>(v).unwrap();
    }
    pub fn write_i16(&mut self, v: i16) {
        self.writer.write_i16::<LittleEndian>(v).unwrap();
    }

    pub fn write_u32(&mut self, v: u32) {
        self.writer.write_u32::<LittleEndian>(v).unwrap();
    }
    pub fn write_i32(&mut self, v: i32) {
        self.writer.write_i32::<LittleEndian>(v).unwrap();
    }

    pub fn write_u64(&mut self, v: u64) {
        self.writer.write_u64::<LittleEndian>(v).unwrap();
    }
    pub fn write_i64(&mut self, v: i64) {
        self.writer.write_i64::<LittleEndian>(v).unwrap();
    }

    pub fn write_usize(&mut self, v: usize) {
        self.writer.write_u64::<LittleEndian>(v as u64).unwrap();
    }
    pub fn write_isize(&mut self, v: isize) {
        self.writer.write_i64::<LittleEndian>(v as i64).unwrap();
    }
    pub fn write_buf(&mut self, v: &[u8]) {
        self.writer.write_all(v).unwrap();
    }
    pub fn write_string(&mut self, v: &str) {
        let asb = v.as_bytes();
        self.write_usize(asb.len());
        self.writer.write_all(asb).unwrap();
    }

    /// Creata a new serializer.
    ///
    /// * `writer` must be an implementatino of [std::io::Write]
    /// * version must be the current version number of the data structures in memory.
    ///   savefile does not support serializing data in any other version number.
    ///   Whenever a field is removed from the protocol, the version number should
    ///   be incremented by 1, and the removed field should be marked with
    ///   a version attribute like:
    ///   `#[versions = "N..M"]`
    ///   Where N is the first version in which the field appear (0 if the field has always existed)
    ///   and M is the version in which the field was removed.
    pub fn store<T:WithSchema + Serialize>(writer: &mut Write, version: u32, data: &T) {
        Self::store_impl(writer,version,data,true);
    }
    pub fn store_noschema<T:WithSchema + Serialize>(writer: &mut Write, version: u32, data: &T) {
        Self::store_impl(writer,version,data,false);
    }
    fn store_impl<T:WithSchema + Serialize>(writer: &mut Write, version: u32, data: &T, with_schema: bool) {
        writer.write_u32::<LittleEndian>(version).unwrap();

        if with_schema
        {
            let schema = T::schema(version);
            let mut schema_serializer=Serializer::new_raw(writer);
            schema.serialize(&mut schema_serializer);            
        }

        let mut serializer=Serializer { writer, version };
        data.serialize(&mut serializer);
    }

    pub fn new_raw(writer: &mut Write) -> Serializer {
        Serializer { writer, version:0 }
    }
}

impl<'a> Deserializer<'a> {
    pub fn read_u8(&mut self) -> u8 {
        let mut buf = [0u8];
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
        let l = self.read_usize();
        let mut v = Vec::with_capacity(l);
        v.resize(l, 0); //TODO: Optimize this
        self.reader.read_exact(&mut v).unwrap();
        String::from_utf8(v).unwrap()
    }

    /// Deserialize an object of type T from the given reader.
    ///
    /// The arguments should be:
    ///  * `reader` A [std::io::Read] object to read serialized bytes from.
    ///  * `version` The version number of the data structures in memory.
    pub fn fetch<T:WithSchema+Deserialize>(reader: &mut Read, version: u32) -> T {
        Deserializer::fetch_impl::<T>(reader,version,true)
    }
    pub fn fetch_noschema<T:WithSchema+Deserialize>(reader: &mut Read, version: u32) -> T {
        Deserializer::fetch_impl::<T>(reader,version,false)
    }
    fn fetch_impl<T:WithSchema+Deserialize>(reader: &mut Read, version: u32, fetch_schema: bool) -> T {
        let file_ver = reader.read_u32::<LittleEndian>().unwrap();
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
            let file_schema = Schema::deserialize(&mut schema_deserializer);
            if file_schema != memory_schema {
                panic!("Saved schema differs from in-memory schema for version {}. File:\n{:?}\nMemory:\n{:?}",file_ver,
                    file_schema,memory_schema);
            }
        }
        let mut deserializer=Deserializer {
            reader,
            file_version: file_ver,
            memory_version: version,
        };
        T::deserialize(&mut deserializer)
    }
    pub fn new_raw(reader: &mut Read) -> Deserializer {
        Deserializer {
            reader,
            file_version: 0,
            memory_version: 0,
        }
    }
}


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
/// #[macro_use]
/// extern crate savefile-derive;
///
/// and the use #[derive(Serialize)]
pub trait Serialize : WithSchema {
    /// Serialize self into the given serializer.
    fn serialize(&self, serializer: &mut Serializer); //TODO: Do error handling

}

/// This trait must be implemented for all data structures you wish to
/// be able to deserialize.
///
/// The most convenient way to implement this is to use
/// #[macro_use]
/// extern crate savefile-derive;
///
/// and the use #[derive(Deserialize)]
pub trait Deserialize : WithSchema {
    /// Deserialize and return an instance of Self from the given deserializer.
    fn deserialize(deserializer: &mut Deserializer) -> Self; //TODO: Do error handling
}


#[derive(Debug,PartialEq)]
pub struct Field {
    pub name : String,
    pub value : Box<Schema>
}

#[derive(Debug,PartialEq)]
pub struct SchemaStruct {
    pub fields : Vec<Field>
}

#[derive(Debug,PartialEq)]
pub struct Variant {
    pub name : String,
    pub discriminator : u16,
    pub fields : Vec<Box<Schema>>
}

#[derive(Debug,PartialEq)]
pub struct SchemaEnum {
    pub variants : Vec<Variant>
}

#[allow(non_camel_case_types)]
#[derive(Debug,PartialEq)]
pub enum SchemaPrimitive {
    schema_i8,
    schema_u8,
    schema_i16,
    schema_u16,
    schema_i32,
    schema_u32,
    schema_i64,
    schema_u64,
    schema_isize,
    schema_usize,
    schema_string
}


/// The schema represents the save file format
/// of your data. 
#[derive(Debug,PartialEq)]
pub enum Schema {
    Struct(SchemaStruct),
    Enum(SchemaEnum),
    Primitive(SchemaPrimitive),
    Vector(Box<Schema>),
    Undefined,
}

impl WithSchema for Field {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for Field {
    fn serialize(&self, serializer: &mut Serializer) {
        serializer.write_string(&self.name);
        self.value.serialize(serializer);
    }
}
impl Deserialize for Field {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        Field {
            name : deserializer.read_string(),
            value : Box::new(Schema::deserialize(deserializer))
        }
    }
}
impl WithSchema for Variant {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for Variant {
    fn serialize(&self, serializer: &mut Serializer) {
        serializer.write_string(&self.name);
        serializer.write_u16(self.discriminator);
        serializer.write_usize(self.fields.len());
        for field in &self.fields  {
            field.serialize(serializer);
        }
    }    
}
impl Deserialize for Variant {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        Variant {
            name: deserializer.read_string(),
            discriminator: deserializer.read_u16(),
            fields : {
                let l = deserializer.read_usize();
                let mut ret=Vec::new();
                for _ in 0..l {
                    ret.push(Box::new(Schema::deserialize(deserializer)))
                }
                ret
            }
        }
    }
}

impl WithSchema for SchemaStruct {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for SchemaStruct {
    fn serialize(&self, serializer: &mut Serializer) {
        serializer.write_usize(self.fields.len());
        for field in &self.fields {
            field.serialize(serializer);
        }
    }
}
impl Deserialize for SchemaStruct {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        let l=deserializer.read_usize();
        SchemaStruct {
            fields: {
                let mut ret=Vec::new();
                for _ in 0..l {
                    ret.push(Field::deserialize(deserializer))
                }
                ret
            }        
        }
    }
}

impl WithSchema for SchemaPrimitive {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for SchemaPrimitive {
    fn serialize(&self, serializer: &mut Serializer) {
        let discr=match *self {
            SchemaPrimitive::schema_i8 => 1,
            SchemaPrimitive::schema_u8 => 2,
            SchemaPrimitive::schema_i16 => 3,
            SchemaPrimitive::schema_u16 => 4,
            SchemaPrimitive::schema_i32 => 5,
            SchemaPrimitive::schema_u32 => 6,
            SchemaPrimitive::schema_i64 => 7,
            SchemaPrimitive::schema_u64 => 8,
            SchemaPrimitive::schema_isize => 9,
            SchemaPrimitive::schema_usize => 10,
            SchemaPrimitive::schema_string => 11,
        };
        serializer.write_u16(discr);
    }
}
impl Deserialize for SchemaPrimitive {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        let var=match deserializer.read_u16() {
            1 => SchemaPrimitive::schema_i8,
            2 => SchemaPrimitive::schema_u8,
            3 => SchemaPrimitive::schema_i16,
            4 => SchemaPrimitive::schema_u16,
            5 => SchemaPrimitive::schema_i32,
            6 => SchemaPrimitive::schema_u32,
            7 => SchemaPrimitive::schema_i64,
            8 => SchemaPrimitive::schema_u64,
            9 => SchemaPrimitive::schema_isize,
            10 => SchemaPrimitive::schema_usize,
            11 => SchemaPrimitive::schema_string,
            c => panic!("Corrupt schema, primitive type #{} encountered",c),
        };
        var
    }
}

impl WithSchema for SchemaEnum {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}

impl Serialize for SchemaEnum {
    fn serialize(&self, serializer: &mut Serializer) {
        serializer.write_usize(self.variants.len());
        for var in &self.variants {
            var.serialize(serializer);
        }
    }
}
impl Deserialize for SchemaEnum {
    fn deserialize(deserializer:&mut Deserializer) -> Self {
        let l = deserializer.read_usize();
        let mut ret=Vec::new();
        for _ in 0..l {
            ret.push(Variant::deserialize(deserializer));
        }
        SchemaEnum {
            variants: ret
        }
    }
}

impl WithSchema for Schema {
    fn schema(_version:u32) -> Schema {
        Schema::Undefined
    }    
}
impl Serialize for Schema {
    fn serialize(&self, serializer: &mut Serializer) {
        match self {
            &Schema::Struct(ref schema_struct) => {
                serializer.write_u16(1);
                schema_struct.serialize(serializer);
            },
            &Schema::Enum(ref schema_enum) => {
                serializer.write_u16(2);
                schema_enum.serialize(serializer);
            },
            &Schema::Primitive(ref schema_prim) => {
                serializer.write_u16(3);
                schema_prim.serialize(serializer);
            },
            &Schema::Vector(ref schema_vector) => {
                serializer.write_u16(4);
                schema_vector.serialize(serializer);
            },
            &Schema::Undefined => {
                serializer.write_u16(5);
            },
        }
    }    
}

impl Deserialize for Schema {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        match deserializer.read_u16() {
            1 => Schema::Struct(SchemaStruct::deserialize(deserializer)),
            2 => Schema::Enum(SchemaEnum::deserialize(deserializer)),
            3 => Schema::Primitive(SchemaPrimitive::deserialize(deserializer)),
            4 => Schema::Vector(Box::new(Schema::deserialize(deserializer))),
            5 => Schema::Undefined,
            c => panic!("Corrupt schema, schema variant had value {}", c),
        }
    }
}

impl WithSchema for String {
    fn schema(_version:u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }    
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


impl<K: WithSchema + Eq + Hash, V: WithSchema, S: ::std::hash::BuildHasher> WithSchema
    for HashMap<K, V, S> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(
            Schema::Struct(SchemaStruct{
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
    fn serialize(&self, serializer: &mut Serializer) {
        serializer.write_usize(self.len());
        for (k, v) in self.iter() {
            k.serialize(serializer);
            v.serialize(serializer);
        }
    }
}

impl<K: Deserialize + Eq + Hash, V: Deserialize> Deserialize for HashMap<K, V> {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        let l = deserializer.read_usize();
        let mut ret = HashMap::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer), V::deserialize(deserializer));
        }
        ret
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
    fn serialize(&self, _serializer: &mut Serializer) {
        panic!("Something is wrong with version-specification of fields - there was an attempt to actually serialize a removed field!");
    }
}
impl<T: WithSchema + Deserialize> Deserialize for Removed<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        T::deserialize(deserializer);
        Removed {
            phantom: std::marker::PhantomData,
        }
    }
}

fn regular_serialize_vec<T: Serialize>(item: &Vec<T>, serializer: &mut Serializer) {
    let l = item.len();
    serializer.write_usize(l);
    for item in item.iter() {
        item.serialize(serializer)
    }
}

impl<T: WithSchema> WithSchema for Vec<T> {
    fn schema(version:u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    default fn serialize(&self, serializer: &mut Serializer) {
        regular_serialize_vec(self, serializer);
    }
}

impl<T: Serialize + ReprC> Serialize for Vec<T> {
    fn serialize(&self, serializer: &mut Serializer) {
        unsafe {
            if !T::repr_c_optimization_safe(serializer.version) {
                regular_serialize_vec(self, serializer);
            } else {
                let l = self.len();
                serializer.write_usize(l);
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ));
            }
        }
    }
}

fn regular_deserialize_vec<T: Deserialize>(deserializer: &mut Deserializer) -> Vec<T> {
    let l = deserializer.read_usize();
    let mut ret = Vec::with_capacity(l);
    for _ in 0..l {
        ret.push(T::deserialize(deserializer));
    }
    ret
}

impl<T: Deserialize> Deserialize for Vec<T> {
    default fn deserialize(deserializer: &mut Deserializer) -> Self {
        regular_deserialize_vec::<T>(deserializer)
    }
}

impl<T: Deserialize + ReprC> Deserialize for Vec<T> {
    fn deserialize(deserializer: &mut Deserializer) -> Self {
        if !T::repr_c_optimization_safe(deserializer.file_version) {
            regular_deserialize_vec::<T>(deserializer)
        } else {
            use std::mem;
            use std::heap::Alloc;
            let align = mem::align_of::<T>();
            let elem_size = mem::size_of::<T>();
            let num_elems = deserializer.read_usize();
            let num_bytes = elem_size * num_elems;
            let layout = alloc::allocator::Layout::from_size_align(num_bytes, align).unwrap();
            let ptr = unsafe { alloc::heap::Heap.alloc(layout.clone()).unwrap() };

            {
                let slice = unsafe { std::slice::from_raw_parts_mut(ptr, num_bytes) };
                match deserializer.reader.read_exact(slice) {
                    Ok(()) => {}
                    _ => {
                        unsafe {
                            alloc::heap::Heap.dealloc(ptr, layout);
                        }
                        panic!("Failed to read from file");
                    }
                }
            }
            unsafe { Vec::from_raw_parts(ptr as *mut T, num_elems, num_elems) }
        }
    }
}
    

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


impl WithSchema for u8 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u8)}}
impl WithSchema for i8 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i8)}}
impl WithSchema for u16 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u16)}}
impl WithSchema for i16 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i16)}}
impl WithSchema for u32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u32)}}
impl WithSchema for i32 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i32)}}
impl WithSchema for u64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_u64)}}
impl WithSchema for i64 {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_i64)}}
impl WithSchema for usize {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_usize)}}
impl WithSchema for isize {fn schema(_version:u32) -> Schema {Schema::Primitive(SchemaPrimitive::schema_isize)}}

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
