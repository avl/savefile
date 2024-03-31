#![allow(incomplete_features)]
#![recursion_limit = "256"]
#![cfg_attr(feature = "nightly", feature(specialization))]
#![deny(missing_docs)]
#![deny(warnings)]

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

# Limitations of Savefile

Savefile does make a few tradeoffs:

1: It only supports the "savefile-format". It does not support any sort of pluggable
architecture with different formats. This format is generally pretty 'raw', data is mostly
formatted the same way as it is in RAM. There is support for bzip2, but this is just a simple
post-processing step.

2: It does not support serializing 'graphs'. I.e, it does not have a concept of object identity,
and cannot handle situations where the same object is reachable through many paths. If two
objects both have a reference to a common object, it will be serialized twice and deserialized
twice.

3: Since it doesn't support 'graphs', it doesn't do well with recursive data structures. When
schema serialization is activated (which is the default), it also doesn't support 'potentially
recursive' data structures. I.e, serializing a tree-object where the same node type can occur
on different levels is not possible, even if the actual links in the tree do not cause any cycles.
This is because the serialized schema is very detailed, and tries to describe exactly what
types may be contained in each node. In a tree, it will determine that children of the node
may be another node, which may itself have children of the same type, which may have children
of the same type, and so on.

# Handling old versions

Let's expand the above example, by creating a 2nd version of the Player struct. Let's say
you decide that your game mechanics don't really need to track the strength of the player, but
you do wish to have a set of skills per player as well as the inventory.

Mark the struct like so:


```
extern crate savefile;
use savefile::prelude::*;
use std::path::Path;
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
    if Path::new("save.bin").exists() == false { /* error handling */ return;}

    let mut player = load_player("save.bin"); //Load from previous save
    assert_eq!("Steve",&player.name); //The name from the previous version saved will remain
    assert_eq!(0,player.skills.len()); //Skills didn't exist when this was saved
    player.skills.push("Whistling".to_string());
    save_player("newsave.bin", &player); //The version saved here will have the vec of skills
}
```


# Behind the scenes

For Savefile to be able to load and save a type T, that type must implement traits
[crate::WithSchema], [crate::Serialize] and [crate::Deserialize] . The custom derive macro Savefile derives
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

The [crate::WithSchema] trait represents a type which knows which data layout it will have
when saved.

## Serialize

The [crate::Serialize] trait represents a type which knows how to write instances of itself to
a `Serializer`.

## Deserialize

The [crate::Deserialize] trait represents a type which knows how to read instances of itself from a `Deserializer`.




# Rules for managing versions

The basic rule is that the Deserialize trait implementation must be able to deserialize data from any previous version.

The WithSchema trait implementation must be able to return the schema for any previous verison.

The Serialize trait implementation only needs to support the latest version.


# Versions and derive

The derive macro used by Savefile supports multiple versions of structs. To make this work,
you have to add attributes whenever fields are removed, added or have their types changed.

When adding or removing fields, use the #\[savefile_versions] attribute.

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

Using the #\[savefile_versions] tag is critically important. If this is messed up, data corruption is likely.

When a field is added, its type must implement the Default trait (unless the default_val or default_fn attributes
are used).

There also exists a savefile_default_val, a default_fn and a savefile_versions_as attribute. More about these below:

## The versions attribute

Rules for using the #\[savefile_versions] attribute:

 You must keep track of what the current version of your data is. Let's call this version N.
 You may only save data using version N (supply this number when calling `save`)
 When data is loaded, you must supply version N as the memory-version number to `load`. Load will
    still adapt the deserialization operation to the version of the serialized data.
 The version number N is "global" (called GLOBAL_VERSION in the previous source example). All components of the saved data must have the same version.
 Whenever changes to the data are to be made, the global version number N must be increased.
 You may add a new field to your structs, iff you also give it a #\[savefile_versions = "N.."] attribute. N must be the new version of your data.
 You may remove a field from your structs. If previously it had no #\[savefile_versions] attribute, you must
    add a #\[savefile_versions = "..N-1"] attribute. If it already had an attribute #[savefile_versions = "M.."], you must close
    its version interval using the current version of your data: #\[savefile_versions = "M..N-1"]. Whenever a field is removed,
    its type must simply be changed to Removed<T> where T is its previous type. You may never completely remove
    items from your structs. Doing so removes backward-compatibility with that version. This will be detected at load.
    For example, if you remove a field in version 3, you should add a #\[savefile_versions="..2"] attribute.
 You may not change the type of a field in your structs, except when using the savefile_versions_as-macro.



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

 ## The savefile_ignore attribute

 The savefile_ignore attribute can be used to exclude certain fields from serialization. They still
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
     #[savefile_ignore]
     cached_product: f64
 }
 # fn main() {}

 ```

 savefile_ignore does not stop the generator from generating an implementation for [Introspect](crate::Introspect) for the given field. To stop
 this as well, also supply the attribute savefile_introspect_ignore .

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
     #[savefile_versions_as="0..0:convert:u64"]
     #[savefile_versions="1.."]
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

 Savefile has an unsafe trait [crate::ReprC] that you must implement for each T. This trait
 has an unsafe function [crate::ReprC::repr_c_optimization_safe] which answers the question:
 - Is this type such that it can safely be copied byte-per-byte?
 Answering yes for a specific type T, causes savefile to optimize serialization of Vec<T> into being
 a very fast, raw memory copy.

 Most of the time, the user doesn't need to implement ReprC, as it can be derived automatically
 by the savefile derive macro.

 However, implementing it manually can be done, but requires care. You, as implementor of the `ReprC`
 trait ()  take full responsibility that all the following rules are upheld:

 * The type T is Copy
 * The host platform is little endian. The savefile disk format uses little endian.
 * The type is represented in memory in an ordered, packed representation. Savefile is not
 clever enough to inspect the actual memory layout and adapt to this, so the memory representation
 has to be all the types of the struct fields in a consecutive sequence without any gaps. Note
 that the #\[repr(C)] attribute is not enough to do this - it will include padding if needed for alignment
 reasons. You should not use #\[repr(packed)], since that may lead to unaligned struct fields.
 Instead, you should use #\[repr(C)] combined with manual padding, if necessary.
 If the type is an enum, it must be #\[repr(u8)]. Enums with fields should work, as long as they
 are #\[repr(u8,C)], but this has not been tested.

 Now, for example, don't do:
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
 detected in release builds. Serializing or deserializing each optimized type at least once somewhere in your test suite
 is recommended.

 When deriving the savefile-traits automatically, specify the attribute ```#[savefile_unsafe_and_fast]``` to require
 the optimized behaviour. If the type doesn't fulfill the required characteristics, a diagnostic will be printed in
 many situations. Using 'savefile_unsafe_and_fast' is not actually unsafe, althought it used to be in an old version.
 Since the speedups it produces are now produced regardless, it is mostly recommended to not use savefile_unsafe_and_fast
 anymore.

 ```
 extern crate savefile;
 use savefile::prelude::*;
 use std::path::Path;

 #[macro_use]
 extern crate savefile_derive;

 #[derive(Clone, Copy, Savefile)]
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

     if Path::new("newsave.bin").exists() == false { /* error handling */ return;}

     let mut player = load_player("newsave.bin"); //Load from previous save
     player.history.push(Position{x:1,y:1});
     player.history.push(Position{x:2,y:1});
     player.history.push(Position{x:2,y:2});
     save_player("newersave.bin", &player);
 }
 ```

 # Custom serialization

 For most user types, the savefile-derive crate can be used to automatically derive serializers
 and deserializers. This is not always possible, however.

 By implementing the traits Serialize, Deserialize and WithSchema, it's possible to create custom
 serializers for any type.

 Let's create a custom serializer for an object MyPathBuf, as an example (this is just an example, because of
 the rust 'orphan rules', only Savefile can actually implement the Savefile-traits for PathBuf. However,
 you can implement the Savefile traits for your own data types in your own crates!)

 The first thing we need to do is implement WithSchema. This trait requires us to return an instance
 of Schema. The Schema is used to 'sanity-check' stored data, so that an attempt to deserialize a
 file which was serialized using a different schema will fail predictably.

 Schema is an enum, with a few built-in variants. See documentation: [crate::Schema] .

 In our case, we choose to handle a MyPathBuf as a string, so we choose Schema::Primitive, with the
 argument SchemaPrimitive::schema_string . If your data is a collection of some sort, Schema::Vector
 may be appropriate.

 Note that the implementor of Serialize and Deserialize have total freedom to serialize data
 to/from the binary stream. The Schema is meant as an extra sanity check, not as an exact format
 specification. The quality of this sanity check will depend on the implementation.



 ````rust
 extern crate savefile;
 pub struct MyPathBuf {
     path: String,
 }
 use savefile::prelude::*;
 impl WithSchema for MyPathBuf {
     fn schema(_version: u32) -> Schema {
         Schema::Primitive(SchemaPrimitive::schema_string)
     }
 }
 impl ReprC for MyPathBuf {
 }
 impl Serialize for MyPathBuf {
     fn serialize<'a>(&self, serializer: &mut Serializer<impl std::io::Write>) -> Result<(), SavefileError> {
         self.path.serialize(serializer)
     }
 }
 impl Deserialize for MyPathBuf {
     fn deserialize(deserializer: &mut Deserializer<impl std::io::Read>) -> Result<Self, SavefileError> {
         Ok(MyPathBuf { path : String::deserialize(deserializer)? } )
     }
 }

 ````


 # Introspection

 The Savefile crate also provides an introspection feature, meant for diagnostics. This is implemented
 through the trait [Introspect](crate::Introspect). Any type implementing this can be introspected.

 The savefile-derive crate supports automatically generating an implementation for most types.

 The introspection is purely 'read only'. There is no provision for using the framework to mutate
 data.

 Here is an example of using the trait directly:


 ````rust
 extern crate savefile;
 #[macro_use]
 extern crate savefile_derive;
 use savefile::Introspect;
 use savefile::IntrospectItem;
 #[derive(Savefile)]
 struct Weight {
     value: u32,
     unit: String
 }
 #[derive(Savefile)]
 struct Person {
     name : String,
     age: u16,
     weight: Weight,
 }
 fn main() {
     let a_person = Person {
         name: "Leo".into(),
         age: 8,
         weight: Weight { value: 26, unit: "kg".into() }
     };
     assert_eq!(a_person.introspect_len(), 3); //There are three fields
     assert_eq!(a_person.introspect_value(), "Person"); //Value of structs is the struct type, per default
     assert_eq!(a_person.introspect_child(0).unwrap().key(), "name"); //Each child has a name and a value. The value is itself a &dyn Introspect, and can be introspected recursively
     assert_eq!(a_person.introspect_child(0).unwrap().val().introspect_value(), "Leo"); //In this case, the child (name) is a simple string with value "Leo".
     assert_eq!(a_person.introspect_child(1).unwrap().key(), "age");
     assert_eq!(a_person.introspect_child(1).unwrap().val().introspect_value(), "8");
     assert_eq!(a_person.introspect_child(2).unwrap().key(), "weight");
     let weight = a_person.introspect_child(2).unwrap();
     assert_eq!(weight.val().introspect_child(0).unwrap().key(), "value"); //Here the child 'weight' has an introspectable weight obj as value
     assert_eq!(weight.val().introspect_child(0).unwrap().val().introspect_value(), "26");
     assert_eq!(weight.val().introspect_child(1).unwrap().key(), "unit");
     assert_eq!(weight.val().introspect_child(1).unwrap().val().introspect_value(), "kg");
 }
 ````

 ## Introspect Details

 By using #\[derive(SavefileIntrospectOnly)] it is possible to have only the Introspect-trait implemented,
 and not the serialization traits. This can be useful for types which aren't possible to serialize,
 but you still wish to have introspection for.

 By using the #\[savefile_introspect_key] attribute on a field, it is possible to make the
 generated [crate::Introspect::introspect_value] return the string representation of the field.
 This can be useful, to have the primary key (name) of an object more prominently visible in the
 introspection output.

 Example:

 ````rust
 # extern crate savefile;
 # #[macro_use]
 # extern crate savefile_derive;
 # use savefile::prelude::*;

 #[derive(Savefile)]
 pub struct StructWithName {
     #[savefile_introspect_key]
     name: String,
     value: String
 }
 # fn main(){}
 ````

 ## Higher level introspection functions

 There is a helper called [crate::Introspector] which allows to get a structured representation
 of parts of an introspectable object. The Introspector has a 'path' which looks in to the
 introspection tree and shows values for this tree. The advantage of using this compared to
 just using ```format!("{:#?}",mystuff)``` is that for very large data structures, unconditionally
 dumping all data may be unwieldy. The author has a struct which becomes hundreds of megabytes
 when formatted using the Debug-trait in this way.

 An example:
 ````rust

 extern crate savefile;
 #[macro_use]
 extern crate savefile_derive;
 use savefile::Introspect;
 use savefile::IntrospectItem;
 use savefile::prelude::*;
 #[derive(Savefile)]
 struct Weight {
     value: u32,
     unit: String
 }
 #[derive(Savefile)]
 struct Person {
     name : String,
     age: u16,
     weight: Weight,
 }
 fn main() {
     let a_person = Person {
         name: "Leo".into(),
         age: 8,
         weight: Weight { value: 26, unit: "kg".into() }
     };

     let mut introspector = Introspector::new();

     let result = introspector.do_introspect(&a_person,
         IntrospectorNavCommand::SelectNth{select_depth:0, select_index: 2}).unwrap();

     println!("{}",result);
     /*
     Output is:

    Introspectionresult:
        name = Leo
        age = 8
        eight = Weight
        value = 26
        unit = kg

      */
     // Note, that there is no point in using the Introspection framework just to get
     // a debug output like above, the point is that for larger data structures, the
     // introspection data can be programmatically used and shown in a live updating GUI,
     // or possibly command line interface or similar. The [crate::IntrospectionResult] does
     // implement Display, but this is just for convenience.

 }


 ````

 ## Navigating using the Introspector

 The [crate::Introspector] object can be used to navigate inside an object being introspected.
 A GUI-program could allow an operator to use arrow keys to navigate the introspected object.

 Every time [crate::Introspector::do_introspect] is called, a [crate::IntrospectorNavCommand] is given
 which can traverse the tree downward or upward. In the example in the previous chapter,
 SelectNth is used to select the 2nd children at the 0th level in the tree.


# Troubleshooting

## The compiler complains that it cannot find item 'deserialize' on a type

Maybe you get an error like:

```the function or associated item `deserialize` exists for struct `Vec<T>`, but its trait bounds were not satisfied```

First, check that you've derived 'Savefile' for the type in question. If you've implemented the Savefile traits
manually, check that you've implemented both ```[crate::prelude::Deserialize]``` and ```[crate::prelude::ReprC]```.
Without ReprC, vectors cannot be deserialized, since savefile can't determine if they are safe to serialize
through simple copying of bytes.


*/

/// The prelude contains all definitions thought to be needed by typical users of the library
pub mod prelude;

extern crate alloc;
#[cfg(feature="arrayvec")]
extern crate arrayvec;
extern crate byteorder;
#[cfg(feature="parking_lot")]
extern crate parking_lot;
#[cfg(feature="smallvec")]
extern crate smallvec;

#[cfg(feature="parking_lot")]
use parking_lot::{Mutex, MutexGuard, RwLock, RwLockReadGuard};

use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::borrow::Cow;
use std::io::Write;
use std::sync::atomic::{
    AtomicBool, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicU16, AtomicU32, AtomicU64, AtomicU8,
    AtomicUsize, Ordering,
};

use self::byteorder::LittleEndian;
use std::collections::BinaryHeap;
use std::collections::VecDeque;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hash;
#[allow(unused_imports)]
use std::mem::MaybeUninit;

#[cfg(feature="indexmap")]
extern crate indexmap;
#[cfg(feature="indexmap")]
use indexmap::{IndexMap, IndexSet};

#[cfg(feature="bit-vec")]
extern crate bit_vec;
#[cfg(feature="bzip2")]
extern crate bzip2;

#[cfg(feature="bit-set")]
extern crate bit_set;

#[cfg(feature="rustc-hash")]
extern crate rustc_hash;
extern crate core;

extern crate memoffset;

#[cfg(feature="derive")]
extern crate savefile_derive;

pub const CURRENT_SAVEFILE_LIB_VERSION:u16 = 0;


/// This object represents an error in deserializing or serializing
/// an item.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub enum SavefileError {
    /// Error given when the schema stored in a file, does not match
    /// the schema given by the data structures in the code, taking into account
    /// versions.
    IncompatibleSchema {
        /// A short description of the incompatibility
        message: String,
    },
    /// Some sort of IO failure. Permissions, broken media etc ...
    IOError {
        /// Cause
        io_error: std::io::Error,
    },
    /// The binary data which is being deserialized, contained an invalid utf8 sequence
    /// where a String was expected. If this occurs, it is either a bug in savefile,
    /// a bug in an implementation of Deserialize, Serialize or WithSchema, or
    /// a corrupt data file.
    InvalidUtf8 {
        /// descriptive message
        msg: String,
    },
    /// Unexpected error with regards to memory layout requirements.
    MemoryAllocationLayoutError,
    /// An Arrayvec had smaller capacity than the size of the data in the binary file.
    ArrayvecCapacityError {
        /// Descriptive message
        msg: String,
    },
    /// The reader returned fewer bytes than expected
    ShortRead,
    /// Cryptographic checksum mismatch. Probably due to a corrupt file.
    CryptographyError,
    /// A persisted value of isize or usize was greater than the maximum for the machine.
    /// This can happen if a file saved by a 64-bit machine contains an usize or isize which
    /// does not fit in a 32 bit word.
    SizeOverflow,
    /// The file does not have a supported version number
    WrongVersion {
        /// Descriptive message
        msg: String,
    },
    /// The file does not have a supported version number
    GeneralError {
        /// Descriptive message
        msg: String,
    },
    /// A poisoned mutex was encountered when traversing the object being saved
    PoisonedMutex,
    /// File was compressed, or user asked for compression, but bzip2-library feature was not enabled.
    CompressionSupportNotCompiledIn,
    /// Invalid char, i.e, a serialized value expected to be a char was encountered, but it had an invalid value.
    InvalidChar,
    /// This occurs for example when using the stable ABI-functionality to call into a library,
    /// and then it turns out that library uses a future, incompatible, Savefile-library version.
    IncompatibleSavefileLibraryVersion,
    /// This occurs if a foreign ABI entry point is missing a method
    MissingMethod {
        /// The name of the missing method
        method_name: String
    },
    /// Savefile ABI only supports at most 64 arguments per function
    TooManyArguments
}

impl Display for SavefileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SavefileError::IncompatibleSchema { message } => {
                write!(f,"Incompatible schema: {}", message)
            }
            SavefileError::IOError { io_error } => {
                write!(f,"IO error: {}", io_error)
            }
            SavefileError::InvalidUtf8 { msg } => {
                write!(f,"Invalid UTF-8: {}", msg)
            }
            SavefileError::MemoryAllocationLayoutError => {
                write!(f,"Memory allocation layout error")
            }
            SavefileError::ArrayvecCapacityError { msg } => {
                write!(f,"Arrayvec capacity error: {}",msg)
            }
            SavefileError::ShortRead => {
                write!(f,"Short read")
            }
            SavefileError::CryptographyError => {
                write!(f,"Cryptography error")
            }
            SavefileError::SizeOverflow => {
                write!(f, "Size overflow")
            }
            SavefileError::WrongVersion { msg } => {
                write!(f, "Wrong version: {}", msg)
            }
            SavefileError::GeneralError { msg } => {
                write!(f, "General error: {}", msg)
            }
            SavefileError::PoisonedMutex => {
                write!(f, "Poisoned mutex")
            }
            SavefileError::CompressionSupportNotCompiledIn => {
                write!(f, "Compression support missing - recompile with bzip2 feature enabled.")
            }
            SavefileError::InvalidChar => {
                write!(f, "Invalid char value encountered.")
            }
        }
    }
}

impl std::error::Error for SavefileError {

}



/// Object to which serialized data is to be written.
/// This is basically just a wrapped `std::io::Write` object
/// and a file protocol version number.
/// In versions prior to 0.15, 'Serializer' did not accept a type parameter.
/// It now requires a type parameter with the type of writer to operate on.
pub struct Serializer<'a, W:Write> {
    /// The underlying writer. You should not access this.
    pub writer: &'a mut W,
    /// The version of the data structures in memory which are being serialized.
    pub version: u32,
}

/// Object from which bytes to be deserialized are read.
/// This is basically just a wrapped `std::io::Read` object,
/// the version number of the file being read, and the
/// current version number of the data structures in memory.
pub struct Deserializer<'a, R: Read> {
    reader: &'a mut R,
    /// The version of the input file
    pub file_version: u32,
    /// The version of the data structures in memory
    pub memory_version: u32,
    /// This contains ephemeral state that can be used to implement de-duplication of
    /// strings or possibly other situations where it is desired to deserialize DAGs.
    ephemeral_state: HashMap<TypeId, Box<dyn Any>>,
}

impl<'a, TR: Read> Deserializer<'a, TR> {
    /// This function constructs a temporary state object of type R, and returns a mutable
    /// reference to it. This object can be used to store data that needs to live for the entire
    /// deserialization session. An example is de-duplicating Arc and other reference counted objects.
    /// Out of the box, Arc<str> has this deduplication done for it.
    /// The type T must be set to the type being deserialized, and is used as a key in a hashmap
    /// separating the state for different types.
    pub fn get_state<T: 'static, R: Default + 'static>(&mut self) -> &mut R {
        let type_id = TypeId::of::<T>();
        let the_any = self
            .ephemeral_state
            .entry(type_id)
            .or_insert_with(|| Box::new(R::default()));

        the_any.downcast_mut().unwrap()
    }
}

/// Marker used to promise that some type fulfills all rules
/// for the "ReprC"-optimization.
#[derive(Default, Debug)]
pub struct IsReprC(bool);

impl std::ops::BitAnd<IsReprC> for IsReprC {
    type Output = IsReprC;

    fn bitand(self, rhs: Self) -> Self::Output {
        IsReprC(self.0 && rhs.0)
    }
}

impl IsReprC {
    /// # SAFETY:
    /// Must only ever be created and immediately returned from
    /// ReprC::repr_c_optimization_safe. Any other use, such
    /// that the value could conceivably be smuggled to
    /// a repr_c_optimization_safe-implementation is forbidden.
    ///
    /// Also, see description of ReprC trait and repr_c_optimization_safe.
    pub unsafe fn yes() -> IsReprC {
        IsReprC(true)
    }
    /// No, the type is not compatible with the "ReprC"-optimization.
    /// It cannot be just blitted.
    /// This is always safe, it just misses out on some optimizations.
    pub fn no() -> IsReprC {
        IsReprC(false)
    }


    /// If this returns false, "ReprC"-Optimization is not allowed.
    #[inline(always)]
    pub fn is_false(self) -> bool {
        !self.0
    }

    /// If this returns true, "ReprC"-Optimization is allowed. Beware.
    #[inline(always)]
    pub fn is_yes(self) -> bool {
        self.0
    }
}

/// This trait describes whether a type is such that it can just be blitted.
/// See method repr_c_optimization_safe.
pub trait ReprC {
    /// This method returns true if the optimization is allowed
    /// for the protocol version given as an argument.
    /// This may return true if and only if the given protocol version
    /// has a serialized format identical to the given protocol version.
    ///
    /// This can return true for types which have an in-memory layout that is packed
    /// and therefore identical to the layout that savefile will use on disk.
    /// This means that types for which this trait is implemented can be serialized
    /// very quickly by just writing their raw bits to disc.
    ///
    /// Rules to allow returning true:
    ///
    /// * The type must be copy
    /// * The type must not contain any padding (if there is padding, backward compatibility will fail, since in fallback mode regular savefile-deserialize will be used, and it will not use padding)
    /// * The type must have a strictly deterministic memory layout (no field order randomization). This typically means repr(C)
    /// * All the constituent types of the type must also implement `ReprC` (correctly).
    ///
    /// Constructing an instance of 'IsReprC' with value 'true' is not safe. See
    /// documentation of 'IsReprC'. The idea is that the ReprC-trait itself
    /// can still be safe to implement, it just won't be possible to get hold of an
    /// instance of IsReprC(true). To make it impossible to just
    /// 'steal' such a value from some other thing implementign 'ReprC',
    /// this method is marked unsafe.
    ///
    /// # SAFETY
    /// The returned value must not be used, except by the Savefile-framework.
    /// It must *not* be be forwarded anywhere else.
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC { IsReprC::no()}
}

impl From<std::io::Error> for SavefileError {
    fn from(s: std::io::Error) -> SavefileError {
        SavefileError::IOError { io_error: s }
    }
}

impl<T> From<std::sync::PoisonError<T>> for SavefileError {
    fn from(_: std::sync::PoisonError<T>) -> SavefileError {
        SavefileError::PoisonedMutex
    }
}

impl From<std::string::FromUtf8Error> for SavefileError {
    fn from(s: std::string::FromUtf8Error) -> SavefileError {
        SavefileError::InvalidUtf8 { msg: s.to_string() }
    }
}
#[cfg(feature="arrayvec")]
impl<T> From<arrayvec::CapacityError<T>> for SavefileError {
    fn from(s: arrayvec::CapacityError<T>) -> SavefileError {
        SavefileError::ArrayvecCapacityError { msg: s.to_string() }
    }
}

impl WithSchema for PathBuf {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }
}
impl Serialize for PathBuf {
    fn serialize<'a>(&self, serializer: &mut Serializer<'a,impl Write>) -> Result<(), SavefileError> {
        let as_string: String = self.to_string_lossy().to_string();
        as_string.serialize(serializer)
    }
}
impl ReprC for PathBuf {}
impl Deserialize for PathBuf {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(PathBuf::from(String::deserialize(deserializer)?))
    }
}
impl Introspect for PathBuf {
    fn introspect_value(&self) -> String {
        self.to_string_lossy().to_string()
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a>>> {
        None
    }
}

impl<'a, T: 'a + WithSchema + ToOwned + ?Sized> WithSchema for Cow<'a, T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<'a, T: 'a + ToOwned +?Sized> ReprC for Cow<'a, T> {}

impl<'a, T: 'a + Serialize + ToOwned + ?Sized> Serialize for Cow<'a, T> {
    fn serialize<'b>(&self, serializer: &mut Serializer<'b, impl Write>) -> Result<(), SavefileError> {
        (**self).serialize(serializer)
    }
}
impl<'a, T: 'a + WithSchema + ToOwned + ?Sized> Deserialize for Cow<'a, T>
where
    T::Owned: Deserialize,
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Cow::Owned(<T as ToOwned>::Owned::deserialize(deserializer)?))
    }
}
impl<'a, T: 'a + Introspect + ToOwned + ?Sized> Introspect for Cow<'a, T> {
    fn introspect_value(&self) -> String {
        (**self).introspect_value()
    }

    fn introspect_child<'b>(&'b self, index: usize) -> Option<Box<dyn IntrospectItem<'b> + 'b>> {
        (**self).introspect_child(index)
    }

    fn introspect_len(&self) -> usize {
        (**self).introspect_len()
    }
}



#[cfg(feature="ring")]
mod crypto {
    use std::fs::File;
    use std::io::{Error, ErrorKind, Read, Write};
    use std::path::Path;
    use ring::aead;
    use ring::aead::{AES_256_GCM, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
    use ring::error::Unspecified;

    extern crate rand;

    use byteorder::{LittleEndian, ReadBytesExt};
    use byteorder::WriteBytesExt;
    use rand::rngs::OsRng;
    use rand::RngCore;
    use crate::{Deserialize, Deserializer, SavefileError, Serialize, Serializer, WithSchema};

    extern crate ring;

    #[derive(Debug)]
    struct RandomNonceSequence {
        data1: u64,
        data2: u32,
    }

    impl RandomNonceSequence {
        pub fn new() -> RandomNonceSequence {
            RandomNonceSequence {
                data1: OsRng.next_u64(),
                data2: OsRng.next_u32(),
            }
        }
        pub fn serialize(&self, writer: &mut dyn Write) -> Result<(), SavefileError> {
            writer.write_u64::<LittleEndian>(self.data1)?;
            writer.write_u32::<LittleEndian>(self.data2)?;
            Ok(())
        }
        pub fn deserialize(reader: &mut dyn Read) -> Result<RandomNonceSequence, SavefileError> {
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
            let mut bytes = [0u8; 12];
            let bytes1: [u8; 8] = unsafe { transmute(self.data1.to_le()) };
            let bytes2: [u8; 4] = unsafe { transmute(self.data2.to_le()) };
            for i in 0..8 {
                bytes[i] = bytes1[i];
            }
            for i in 0..4 {
                bytes[i + 8] = bytes2[i];
            }

            Ok(Nonce::assume_unique_for_key(bytes))
        }
    }

    /// A cryptographic stream wrapper.
    /// Wraps a plain dyn Write, and itself implements Write, encrypting
    /// all data written.
    pub struct CryptoWriter<'a> {
        writer: &'a mut dyn Write,
        buf: Vec<u8>,
        sealkey: SealingKey<RandomNonceSequence>,
        failed: bool,
    }

    /// A cryptographic stream wrapper.
    /// Wraps a plain dyn Read, and itself implements Read, decrypting
    /// and verifying all data read.
    pub struct CryptoReader<'a> {
        reader: &'a mut dyn Read,
        buf: Vec<u8>,
        offset: usize,
        openingkey: OpeningKey<RandomNonceSequence>,
    }

    impl<'a> CryptoReader<'a> {
        /// Create a new CryptoReader, wrapping the given Read . Decrypts using the given
        /// 32 byte cryptographic key.
        /// Crypto is 256 bit AES GCM
        pub fn new(reader: &'a mut dyn Read, key_bytes: [u8; 32]) -> Result<CryptoReader<'a>, SavefileError> {
            let unboundkey = UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap();

            let nonce_sequence = RandomNonceSequence::deserialize(reader)?;
            let openingkey = OpeningKey::new(unboundkey, nonce_sequence);

            Ok(CryptoReader {
                reader,
                offset: 0,
                buf: Vec::new(),
                openingkey,
            })
        }
    }

    const CRYPTO_BUFSIZE: usize = 100_000;

    impl<'a> Drop for CryptoWriter<'a> {
        fn drop(&mut self) {
            self.flush().expect("The implicit flush in the Drop of CryptoWriter failed. This causes this panic. If you want to be able to handle this, make sure to call flush() manually. If a manual flush has failed, Drop won't panic.");
        }
    }

    impl<'a> CryptoWriter<'a> {
        /// Create a new CryptoWriter, wrapping the given Write . Encrypts using the given
        /// 32 byte cryptographic key.
        /// Crypto is 256 bit AES GCM
        pub fn new(writer: &'a mut dyn Write, key_bytes: [u8; 32]) -> Result<CryptoWriter<'a>, SavefileError> {
            let unboundkey = UnboundKey::new(&AES_256_GCM, &key_bytes).unwrap();
            let nonce_sequence = RandomNonceSequence::new();
            nonce_sequence.serialize(writer)?;
            let sealkey = SealingKey::new(unboundkey, nonce_sequence);
            Ok(CryptoWriter {
                writer,
                buf: Vec::new(),
                sealkey,
                failed: false,
            })
        }
        /// Data is encrypted in chunks. Calling this unconditionally finalizes a chunk, actually emitting
        /// data to the underlying dyn Write. When later reading data, an entire chunk must be read
        /// from file before any plaintext is produced.
        pub fn flush_final(mut self) -> Result<(), SavefileError> {
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
                    buf.clone_from_slice(&self.buf[self.offset..self.offset + buf.len()]);
                    self.offset += buf.len();
                    return Ok(buf.len());
                }

                {
                    let oldlen = self.buf.len();
                    let newlen = self.buf.len() - self.offset;
                    self.buf.copy_within(self.offset..oldlen, 0);
                    self.buf.resize(newlen, 0);
                    self.offset = 0;
                }
                let mut sizebuf = [0; 8];
                let mut sizebuf_bytes_read = 0;
                loop {
                    match self.reader.read(&mut sizebuf[sizebuf_bytes_read..]) {
                        Ok(gotsize) => {
                            if gotsize == 0 {
                                if sizebuf_bytes_read == 0 {
                                    let cur_content_size = self.buf.len() - self.offset;
                                    buf[0..cur_content_size]
                                        .clone_from_slice(&self.buf[self.offset..self.offset + cur_content_size]);
                                    self.offset += cur_content_size;
                                    return Ok(cur_content_size);
                                } else {
                                    return Err(Error::new(ErrorKind::UnexpectedEof, "Unexpected EOF"));
                                }
                            } else {
                                sizebuf_bytes_read += gotsize;
                                assert!(sizebuf_bytes_read <= 8);
                            }
                        }
                        Err(err) => return Err(err),
                    }
                    if sizebuf_bytes_read == 8 {
                        break;
                    }
                }
                use byteorder::ByteOrder;
                let curlen = byteorder::LittleEndian::read_u64(&sizebuf) as usize;

                if curlen > CRYPTO_BUFSIZE + 16 {
                    return Err(Error::new(ErrorKind::Other, "Cryptography error"));
                }
                let orglen = self.buf.len();
                self.buf.resize(orglen + curlen, 0);

                self.reader.read_exact(&mut self.buf[orglen..orglen + curlen])?;

                match self
                    .openingkey
                    .open_in_place(aead::Aad::empty(), &mut self.buf[orglen..])
                {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(Error::new(ErrorKind::Other, "Cryptography error"));
                    }
                }
                self.buf.resize(self.buf.len() - 16, 0);
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
        /// lost.
        fn flush(&mut self) -> Result<(), Error> {
            self.failed = true;
            let mut offset = 0;

            let mut tempbuf = Vec::new();
            if self.buf.len() > CRYPTO_BUFSIZE {
                tempbuf = Vec::<u8>::with_capacity(CRYPTO_BUFSIZE + 16);
            }

            while self.buf.len() > offset {
                let curbuf;
                if offset == 0 && self.buf.len() <= CRYPTO_BUFSIZE {
                    curbuf = &mut self.buf;
                } else {
                    let chunksize = (self.buf.len() - offset).min(CRYPTO_BUFSIZE);
                    tempbuf.resize(chunksize, 0u8);
                    tempbuf.clone_from_slice(&self.buf[offset..offset + chunksize]);
                    curbuf = &mut tempbuf;
                }
                let expected_final_len = curbuf.len() as u64 + 16;
                debug_assert!(expected_final_len <= CRYPTO_BUFSIZE as u64 + 16);

                self.writer.write_u64::<LittleEndian>(expected_final_len)?; //16 for the tag
                match self.sealkey.seal_in_place_append_tag(aead::Aad::empty(), curbuf) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(Error::new(ErrorKind::Other, "Cryptography error"));
                    }
                }
                debug_assert!(curbuf.len() == expected_final_len as usize, "The size of the TAG generated by the AES 256 GCM in ring seems to have changed! This is very unexpected. File a bug on the savefile-crate");

                self.writer.write_all(&curbuf[..])?;
                self.writer.flush()?;
                offset += curbuf.len() - 16;
                curbuf.resize(curbuf.len() - 16, 0);
            }
            self.buf.clear();
            self.failed = false;
            Ok(())
        }
    }
    /// Like [crate::save_file], except encrypts the data with AES256, using the SHA256 hash
    /// of the password as key.
    pub fn save_encrypted_file<T: WithSchema + Serialize, P:AsRef<Path>>(
        filepath: P,
        version: u32,
        data: &T,
        password: &str,
    ) -> Result<(), SavefileError> {
        use ring::digest;
        let actual = digest::digest(&digest::SHA256, password.as_bytes());
        let mut key = [0u8; 32];
        let password_hash = actual.as_ref();
        assert_eq!(password_hash.len(), key.len(), "A SHA256 sum must be 32 bytes");
        key.clone_from_slice(password_hash);

        let mut f = File::create(filepath)?;
        let mut writer = CryptoWriter::new(&mut f, key)?;

        Serializer::<CryptoWriter>::save::<T>(&mut writer, version, data, true)?;
        writer.flush()?;
        Ok(())
    }

    /// Like [crate::load_file], except it expects the file to be an encrypted file previously stored using
    /// [crate::save_encrypted_file].
    pub fn load_encrypted_file<T: WithSchema + Deserialize, P:AsRef<Path>>(
        filepath: P,
        version: u32,
        password: &str,
    ) -> Result<T, SavefileError> {
        use ring::digest;
        let actual = digest::digest(&digest::SHA256, password.as_bytes());
        let mut key = [0u8; 32];
        let password_hash = actual.as_ref();
        assert_eq!(password_hash.len(), key.len(), "A SHA256 sum must be 32 bytes");
        key.clone_from_slice(password_hash);

        let mut f = File::open(filepath)?;
        let mut reader = CryptoReader::new(&mut f, key).unwrap();
        Deserializer::<CryptoReader>::load::<T>(&mut reader, version)
    }
}
#[cfg(feature="ring")]
pub use crypto::{CryptoReader, CryptoWriter, load_encrypted_file, save_encrypted_file};

impl<'a, W:Write+'a> Serializer<'a, W> {
    /// Writes a binary bool to the output
    #[inline(always)]
    pub fn write_bool(&mut self, v: bool) -> Result<(), SavefileError> {
        Ok(self.writer.write_u8(if v { 1 } else { 0 })?)
    }
    /// Writes a binary u8 to the output
    #[inline(always)]
    pub fn write_u8(&mut self, v: u8) -> Result<(), SavefileError> {
        Ok(self.writer.write_all(&[v])?)
    }
    /// Writes a binary i8 to the output
    #[inline(always)]
    pub fn write_i8(&mut self, v: i8) -> Result<(), SavefileError> {
        Ok(self.writer.write_i8(v)?)
    }

    /// Writes a binary little endian u16 to the output
    #[inline(always)]
    pub fn write_u16(&mut self, v: u16) -> Result<(), SavefileError> {
        Ok(self.writer.write_u16::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian i16 to the output
    #[inline(always)]
    pub fn write_i16(&mut self, v: i16) -> Result<(), SavefileError> {
        Ok(self.writer.write_i16::<LittleEndian>(v)?)
    }

    /// Writes a binary little endian u32 to the output
    #[inline(always)]
    pub fn write_u32(&mut self, v: u32) -> Result<(), SavefileError> {
        Ok(self.writer.write_u32::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian i32 to the output
    #[inline(always)]
    pub fn write_i32(&mut self, v: i32) -> Result<(), SavefileError> {
        Ok(self.writer.write_i32::<LittleEndian>(v)?)
    }

    /// Writes a binary little endian f32 to the output
    #[inline(always)]
    pub fn write_f32(&mut self, v: f32) -> Result<(), SavefileError> {
        Ok(self.writer.write_f32::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian f64 to the output
    #[inline(always)]
    pub fn write_f64(&mut self, v: f64) -> Result<(), SavefileError> {
        Ok(self.writer.write_f64::<LittleEndian>(v)?)
    }

    /// Writes a binary little endian u64 to the output
    #[inline(always)]
    pub fn write_u64(&mut self, v: u64) -> Result<(), SavefileError> {
        Ok(self.writer.write_u64::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian i64 to the output
    #[inline(always)]
    pub fn write_i64(&mut self, v: i64) -> Result<(), SavefileError> {
        Ok(self.writer.write_i64::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian u128 to the output
    #[inline(always)]
    pub fn write_u128(&mut self, v: u128) -> Result<(), SavefileError> {
        Ok(self.writer.write_u128::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian i128 to the output
    #[inline(always)]
    pub fn write_i128(&mut self, v: i128) -> Result<(), SavefileError> {
        Ok(self.writer.write_i128::<LittleEndian>(v)?)
    }
    /// Writes a binary little endian usize as u64 to the output
    #[inline(always)]
    pub fn write_usize(&mut self, v: usize) -> Result<(), SavefileError> {
        Ok(self.writer.write_u64::<LittleEndian>(v as u64)?)
    }
    /// Writes a binary little endian isize as i64 to the output
    #[inline(always)]
    pub fn write_isize(&mut self, v: isize) -> Result<(), SavefileError> {
        Ok(self.writer.write_i64::<LittleEndian>(v as i64)?)
    }
    /// Writes a binary u8 array to the output
    #[inline(always)]
    pub fn write_buf(&mut self, v: &[u8]) -> Result<(), SavefileError> {
        Ok(self.writer.write_all(v)?)
    }
    /// Writes as a string as 64 bit length + utf8 data
    #[inline(always)]
    pub fn write_string(&mut self, v: &str) -> Result<(), SavefileError> {
        let asb = v.as_bytes();
        self.write_usize(asb.len())?;
        Ok(self.writer.write_all(asb)?)
    }
    /// Writes a binary u8 array to the output. Synonym of write_buf.
    #[inline(always)]
    pub fn write_bytes(&mut self, v: &[u8]) -> Result<(), SavefileError> {
        Ok(self.writer.write_all(v)?)
    }

    /// Writes an interval of memory to the output
    /// #SAFETY:
    /// All the memory between the start of t1 and up to the end of t2 must
    /// be contiguous, without padding and safe to transmute to a u8-slice([u8]).
    /// The memory must all be within the object pointed to by full.
    /// The 'full' object is only needed to satisfy miri, otherwise
    /// we violate the rules when we create one continuous thing from parts.
    #[inline(always)]
    #[doc(hidden)]
    pub unsafe fn raw_write_region<T, T1:ReprC,T2:ReprC>(&mut self, full: &T, t1: &T1, t2: &T2, version: u32) -> Result<(), SavefileError> {
        assert!(T1::repr_c_optimization_safe(version).is_yes());
        assert!(T2::repr_c_optimization_safe(version).is_yes());


        let base = full as *const T as *const u8;
        let totlen = std::mem::size_of::<T>();
        let p1 = (t1 as *const T1 as *const u8) as usize;
        let p2 = (t2 as *const T2 as *const u8) as usize;
        let start = p1 - (base as usize);
        let end = (p2 - (base as usize)) + std::mem::size_of::<T2>();
        let full_slice = std::slice::from_raw_parts(base, totlen);
        Ok(self.writer.write_all(&full_slice[start..end])?)

    }
    /// Creata a new serializer.
    /// Don't use this function directly, use the [crate::save] function instead.
    pub fn save<T: WithSchema + Serialize>(
        writer: &mut W,
        version: u32,
        data: &T,
        with_compression: bool,
    ) -> Result<(), SavefileError> {
        Ok(Self::save_impl(writer, version, data, true, with_compression)?)
    }
    /// Creata a new serializer.
    /// Don't use this function directly, use the [crate::save_noschema] function instead.
    pub fn save_noschema<T: WithSchema + Serialize>(
        writer: &mut W,
        version: u32,
        data: &T,
    ) -> Result<(), SavefileError> {
        Ok(Self::save_impl(writer, version, data, false, false)?)
    }
    fn save_impl<T: WithSchema + Serialize>(
        writer: &mut W,
        version: u32,
        data: &T,
        with_schema: bool,
        with_compression: bool,
    ) -> Result<(), SavefileError> {
        let header = "savefile\0".to_string().into_bytes();

        writer.write_all(&header)?; //9

        writer.write_u16::<LittleEndian>(0 /*savefile format version*/)?;
        writer.write_u32::<LittleEndian>(version)?;
        // 9 + 2 + 4 = 15

        {

            if with_compression {
                writer.write_u8(1)?; //15 + 1 = 16

                #[cfg(feature="bzip2")]
                    {
                        let mut compressed_writer = bzip2::write::BzEncoder::new(writer, Compression::best());
                        if with_schema {
                            let schema = T::schema(version);
                            let mut schema_serializer = Serializer::<bzip2::write::BzEncoder<W>>::new_raw(&mut compressed_writer);
                            schema.serialize(&mut schema_serializer)?;
                        }

                        let mut serializer = Serializer { writer: &mut compressed_writer, version };
                        data.serialize(&mut serializer)?;
                        compressed_writer.flush()?;
                        return Ok(())

                    }
                #[cfg(not(feature="bzip2"))]
                    {
                        return Err(SavefileError::CompressionSupportNotCompiledIn);
                    }

            } else {
                writer.write_u8(0)?;
                if with_schema {
                    let schema = T::schema(version);
                    let mut schema_serializer = Serializer::<W>::new_raw(writer);
                    schema.serialize(&mut schema_serializer)?;
                }

                let mut serializer = Serializer { writer, version };
                data.serialize(&mut serializer)?;
                writer.flush()?;
                Ok(())
            }
        }

    }

    /// Create a Serializer.
    /// Don't use this method directly, use the [crate::save] function
    /// instead.
    pub fn new_raw(writer: &mut impl Write) -> Serializer<impl Write> {
        Serializer { writer, version: 0 }
    }
}

impl<'a, TR:Read> Deserializer<'a, TR> {
    /// Reads a u8 and return true if equal to 1
    pub fn read_bool(&mut self) -> Result<bool, SavefileError> {
        Ok(self.reader.read_u8()? == 1)
    }
    /// Reads an u8
    pub fn read_u8(&mut self) -> Result<u8, SavefileError> {
        let mut buf = [0u8];
        self.reader.read_exact(&mut buf)?;
        Ok(buf[0])
    }
    /// Reads a little endian u16
    pub fn read_u16(&mut self) -> Result<u16, SavefileError> {
        Ok(self.reader.read_u16::<LittleEndian>()?)
    }
    /// Reads a little endian u32
    pub fn read_u32(&mut self) -> Result<u32, SavefileError> {
        Ok(self.reader.read_u32::<LittleEndian>()?)
    }
    /// Reads a little endian u64
    pub fn read_u64(&mut self) -> Result<u64, SavefileError> {
        Ok(self.reader.read_u64::<LittleEndian>()?)
    }
    /// Reads a little endian u128
    pub fn read_u128(&mut self) -> Result<u128, SavefileError> {
        Ok(self.reader.read_u128::<LittleEndian>()?)
    }
    /// Reads an i8
    pub fn read_i8(&mut self) -> Result<i8, SavefileError> {
        Ok(self.reader.read_i8()?)
    }
    /// Reads a little endian i16
    pub fn read_i16(&mut self) -> Result<i16, SavefileError> {
        Ok(self.reader.read_i16::<LittleEndian>()?)
    }
    /// Reads a little endian i32
    pub fn read_i32(&mut self) -> Result<i32, SavefileError> {
        Ok(self.reader.read_i32::<LittleEndian>()?)
    }
    /// Reads a little endian i64
    pub fn read_i64(&mut self) -> Result<i64, SavefileError> {
        Ok(self.reader.read_i64::<LittleEndian>()?)
    }
    /// Reads a little endian i128
    pub fn read_i128(&mut self) -> Result<i128, SavefileError> {
        Ok(self.reader.read_i128::<LittleEndian>()?)
    }
    /// Reads a little endian f32
    pub fn read_f32(&mut self) -> Result<f32, SavefileError> {
        Ok(self.reader.read_f32::<LittleEndian>()?)
    }
    /// Reads a little endian f64
    pub fn read_f64(&mut self) -> Result<f64, SavefileError> {
        Ok(self.reader.read_f64::<LittleEndian>()?)
    }
    /// Reads an i64 into an isize. For 32 bit architectures, the function fails on overflow.
    pub fn read_isize(&mut self) -> Result<isize, SavefileError> {
        if let Ok(val) = TryFrom::try_from(self.reader.read_i64::<LittleEndian>()? as isize) {
            Ok(val)
        } else {
            Err(SavefileError::SizeOverflow)
        }
    }
    /// Reads an u64 into an usize. For 32 bit architectures, the function fails on overflow.
    pub fn read_usize(&mut self) -> Result<usize, SavefileError> {
        if let Ok(val) = TryFrom::try_from(self.reader.read_u64::<LittleEndian>()? as usize) {
            Ok(val)
        } else {
            Err(SavefileError::SizeOverflow)
        }
    }
    /// Reads a 64 bit length followed by an utf8 encoded string. Fails if data is not valid utf8
    pub fn read_string(&mut self) -> Result<String, SavefileError> {
        let l = self.read_usize()?;
        #[cfg(feature = "size_sanity_checks")]
        {
            if l > 1_000_000 {
                return Err(SavefileError::GeneralError {
                    msg: format!("String too large"),
                });
            }
        }
        let mut v = Vec::with_capacity(l);
        v.resize(l, 0); //TODO: Optimize this
        self.reader.read_exact(&mut v)?;
        Ok(String::from_utf8(v)?)
    }

    /// Reads 'len' raw u8 bytes as a Vec<u8>
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, SavefileError> {
        let mut v = Vec::with_capacity(len);
        v.resize(len, 0); //TODO: Optimize this
        self.reader.read_exact(&mut v)?;
        Ok(v)
    }
    /// Reads raw u8 bytes into the given buffer. The buffer size must be
    /// equal to the number of bytes desired to be read.
    pub fn read_bytes_to_buf(&mut self, buf: &mut [u8]) -> Result<(), SavefileError> {
        self.reader.read_exact(buf)?;
        Ok(())
    }

    /// Deserialize an object of type T from the given reader.
    /// Don't use this method directly, use the [crate::load] function
    /// instead.
    pub fn load<T: WithSchema + Deserialize>(reader: &mut TR, version: u32) -> Result<T, SavefileError> {
        Deserializer::<_>::load_impl::<T>(reader, version, true)
    }

    /// Deserialize an object of type T from the given reader.
    /// Don't use this method directly, use the [crate::load_noschema] function
    /// instead.
    pub fn load_noschema<T: WithSchema + Deserialize>(reader: &mut TR, version: u32) -> Result<T, SavefileError> {
        Deserializer::<TR>::load_impl::<T>(reader, version, false)
    }
    fn load_impl<T: WithSchema + Deserialize>(
        reader: &mut TR,
        version: u32,
        fetch_schema: bool,
    ) -> Result<T, SavefileError> {
        let mut head: [u8; 9] = [0u8; 9];
        reader.read_exact(&mut head)?;

        if &head[..] != &("savefile\0".to_string().into_bytes())[..] {
            return Err(SavefileError::GeneralError {msg: "File is not in new savefile-format.".into()});
        }

        let savefile_lib_version = reader.read_u16::<LittleEndian>()?;
        if savefile_lib_version != CURRENT_SAVEFILE_LIB_VERSION { //Note, in future we might interpret this as 'schema version', thus allowing newer code to read files with older versions of the schema-definition
            return Err(SavefileError::GeneralError {msg: "This file has been created by an earlier, incompatible version of the savefile crate (0.5.0 or before).".into()});
        }
        let file_ver = reader.read_u32::<LittleEndian>()?;

        if file_ver > version {
            return Err(SavefileError::WrongVersion {
                msg: format!(
                    "File has later version ({}) than structs in memory ({}).",
                    file_ver, version
                ),
            });
        }
        let with_compression = reader.read_u8()? != 0;

        if with_compression {
            #[cfg(feature="bzip2")]
                {
                    let mut compressed_reader = bzip2::read::BzDecoder::new(reader);
                    if fetch_schema {
                        let mut schema_deserializer = Deserializer::<bzip2::read::BzDecoder<TR>>::new_schema_deserializer(&mut compressed_reader, CURRENT_SAVEFILE_LIB_VERSION);
                        let memory_schema = T::schema(file_ver);
                        let file_schema = Schema::deserialize(&mut schema_deserializer)?;

                        if let Some(err) = diff_schema(&memory_schema, &file_schema, ".".to_string()) {
                            return Err(SavefileError::IncompatibleSchema {
                                message: format!(
                                    "Saved schema differs from in-memory schema for version {}. Error: {}",
                                    file_ver, err
                                ),
                            });
                        }
                    }
                    let mut deserializer = Deserializer {
                        reader: &mut compressed_reader,
                        file_version: file_ver,
                        memory_version: version,
                        ephemeral_state: HashMap::new(),
                    };
                    Ok(T::deserialize(&mut deserializer)?)
                }
            #[cfg(not(feature="bzip2"))]
                {
                    return Err(SavefileError::CompressionSupportNotCompiledIn);
                }
        } else {
            if fetch_schema {
                let mut schema_deserializer = Deserializer::<TR>::new_schema_deserializer(reader, CURRENT_SAVEFILE_LIB_VERSION);
                let memory_schema = T::schema(file_ver);
                let file_schema = Schema::deserialize(&mut schema_deserializer)?;

                if let Some(err) = diff_schema(&memory_schema, &file_schema, ".".to_string()) {
                    return Err(SavefileError::IncompatibleSchema {
                        message: format!(
                            "Saved schema differs from in-memory schema for version {}. Error: {}",
                            file_ver, err
                        ),
                    });
                }
            }
            let mut deserializer = Deserializer {
                reader,
                file_version: file_ver,
                memory_version: version,
                ephemeral_state: HashMap::new(),
            };
            Ok(T::deserialize(&mut deserializer)?)
        }

    }

    /// Create a Deserializer.
    /// Don't use this method directly, use the [crate::load] function
    /// instead.
    pub fn new_schema_deserializer(reader: &mut impl Read, file_schema_version: u16) -> Deserializer<impl Read> {
        Deserializer {
            reader,
            file_version: file_schema_version as u32,
            memory_version: CURRENT_SAVEFILE_LIB_VERSION as u32,
            ephemeral_state: HashMap::new(),
        }
    }
}

/// Deserialize an instance of type T from the given `reader` .
/// The current type of T in memory must be equal to `version`.
/// The deserializer will use the actual protocol version in the
/// file to do the deserialization.
pub fn load<T: WithSchema + Deserialize>(reader: &mut impl Read, version: u32) -> Result<T, SavefileError> {
    Deserializer::<_>::load::<T>(reader, version)
}

/// Deserialize an instance of type T from the given u8 slice .
/// The current type of T in memory must be equal to `version`.
/// The deserializer will use the actual protocol version in the
/// file to do the deserialization.
pub fn load_from_mem<T: WithSchema + Deserialize>(input: &[u8], version: u32) -> Result<T, SavefileError> {
    let mut input = input;
    Deserializer::load::<T>(&mut input, version)
}

/// Write the given `data` to the `writer`.
/// The current version of data must be `version`.
pub fn save<T: WithSchema + Serialize>(writer: &mut impl Write, version: u32, data: &T) -> Result<(), SavefileError> {
    Serializer::save::<T>(writer, version, data, false)
}

/// Write the given `data` to the `writer`. Compresses data using 'bzip2' compression format.
/// The current version of data must be `version`.
/// The resultant data can be loaded using the regular load-function (it autodetects if compressions was
/// active or not).
/// Note, this function will fail if the bzip2-feature is not enabled.
pub fn save_compressed<T: WithSchema + Serialize>(
    writer: &mut impl Write,
    version: u32,
    data: &T,
) -> Result<(), SavefileError> {
    Serializer::save::<T>(writer, version, data, true)
}

/// Serialize the given data and return as a Vec<u8>
/// The current version of data must be `version`.
pub fn save_to_mem<T: WithSchema + Serialize>(version: u32, data: &T) -> Result<Vec<u8>, SavefileError> {
    let mut retval = Vec::new();
    Serializer::save::<T>(&mut retval, version, data, false)?;
    Ok(retval)
}

/// Like [crate::load] , but used to open files saved without schema,
/// by one of the _noschema versions of the save functions.
pub fn load_noschema<T: WithSchema + Deserialize>(reader: &mut impl Read, version: u32) -> Result<T, SavefileError> {
    Deserializer::<_>::load_noschema::<T>(reader, version)
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
pub fn save_noschema<T: WithSchema + Serialize>(
    writer: &mut impl Write,
    version: u32,
    data: &T,
) -> Result<(), SavefileError> {
    Serializer::save_noschema::<T>(writer, version, data)
}

/// Like [crate::load] , except it deserializes from the given file in the filesystem.
/// This is a pure convenience function.
pub fn load_file<T: WithSchema + Deserialize,P:AsRef<Path>>(filepath: P, version: u32) -> Result<T, SavefileError> {
    let mut f = BufReader::new(File::open(filepath)?);
    Deserializer::load::<T>(&mut f, version)
}

/// Like [crate::save] , except it opens a file on the filesystem and writes
/// the data to it. This is a pure convenience function.
pub fn save_file<T: WithSchema + Serialize, P:AsRef<Path>>(filepath: P, version: u32, data: &T) -> Result<(), SavefileError> {
    let mut f =  BufWriter::new(File::create(filepath)?);
    Serializer::save::<T>(&mut f, version, data, false)
}

/// Like [crate::load_noschema] , except it deserializes from the given file in the filesystem.
/// This is a pure convenience function.
pub fn load_file_noschema<T: WithSchema + Deserialize, P: AsRef<Path>>(filepath: P, version: u32) -> Result<T, SavefileError> {
    let mut f = BufReader::new(File::open(filepath)?);
    Deserializer::load_noschema::<T>(&mut f, version)
}

/// Like [crate::save_noschema] , except it opens a file on the filesystem and writes
/// the data to it. This is a pure convenience function.
pub fn save_file_noschema<T: WithSchema + Serialize, P:AsRef<Path>>(
    filepath: P,
    version: u32,
    data: &T,
) -> Result<(), SavefileError> {
    let mut f = BufWriter::new(File::create(filepath)?);
    Serializer::save_noschema::<T>(&mut f, version, data)
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
    fn schema(version: u32) -> Schema;
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
/// and the use #\[derive(Serialize)]
pub trait Serialize: WithSchema {
    /// Serialize self into the given serializer.
    /// In versions prior to 0.15, 'Serializer' did not accept a type parameter.
    /// It now requires a type parameter with the type of writer expected.
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError>; //TODO: Do error handling
}

/// A child of an object implementing Introspect. Is a key-value pair. The only reason this is not
/// simply (String, &dyn Introspect) is that Mutex wouldn't be introspectable in that case.
/// Mutex needs something like (String, MutexGuard<T>). By having this a trait,
/// different types can have whatever reference holder needed (MutexGuard, RefMut etc).
pub trait IntrospectItem<'a> {
    /// Should return a descriptive string for the given child. For structures,
    /// this would be the field name, for instance.
    fn key(&self) -> &str;
    /// The introspectable value of the child.
    fn val(&self) -> &dyn Introspect;
}

/// This is an zero-sized introspectable object with no value and no children.
/// It is used for situations where you wish to have a key but no value.
struct NullIntrospectable {}
static THE_NULL_INTROSPECTABLE: NullIntrospectable = NullIntrospectable{};

impl Introspect for NullIntrospectable {
    fn introspect_value(&self) -> String {
        String::new()
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
    fn introspect_len(&self) -> usize {
        0
    }
}
impl<'a> IntrospectItem<'a> for String {
    fn key(&self) -> &str {
        &self
    }

    fn val(&self) -> &dyn Introspect {
        &THE_NULL_INTROSPECTABLE
    }
}

/// As a sort of guard against infinite loops, the default 'len'-implementation only
/// ever iterates this many times. This is so that broken 'introspect_child'-implementations
/// won't cause introspect_len to iterate forever.
pub const MAX_CHILDREN: usize = 10000;

/// Gives the ability to look into an object, inspecting any children (fields).
pub trait Introspect {
    /// Returns the value of the object, excluding children, as a string.
    /// Exactly what the value returned here is depends on the type.
    /// For some types, like a plain array, there isn't much of a value,
    /// the entire information of object resides in the children.
    /// For other cases, like a department in an organisation, it might
    /// make sense to have the value be the name, and have all the other properties
    /// as children.
    fn introspect_value(&self) -> String;

    /// Returns an the name and &dyn Introspect for the child with the given index,
    /// or if no child with that index exists, None.
    /// All the children should be indexed consecutively starting at 0 with no gaps,
    /// all though there isn't really anything stopping the user of the trait to have
    /// any arbitrary index strategy, consecutive numbering 0, 1, 2, ... etc is strongly
    /// encouraged.
    fn introspect_child<'a>(&'a self, index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>>;

    /// Returns the total number of children.
    /// The default implementation calculates this by simply calling introspect_child with
    /// higher and higher indexes until it returns None.
    /// It gives up if the count reaches 10000. If your type can be bigger
    /// and you want to be able to introspect it, override this method.
    fn introspect_len(&self) -> usize {
        for child_index in 0..MAX_CHILDREN {
            if self.introspect_child(child_index).is_none() {
                return child_index;
            }
        }
        return MAX_CHILDREN;
    }
}

/// This trait must be implemented for all data structures you wish to
/// be able to deserialize.
///
/// The most convenient way to implement this is to use
/// `#[macro_use]
/// extern crate savefile-derive;`
///
/// and the use #\[derive(Deserialize)]
pub trait Deserialize: WithSchema + Sized {
    /// Deserialize and return an instance of Self from the given deserializer.
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError>; //TODO: Do error handling
}

/// A field is serialized according to its value.
/// The name is just for diagnostics.
#[derive(Debug, PartialEq)]
pub struct Field {
    /// Field name
    pub name: String,
    /// Field type
    pub value: Box<Schema>,
}

/// An array is serialized by serializing its items one by one,
/// without any padding.
/// The dbg_name is just for diagnostics.
#[derive(Debug, PartialEq)]
pub struct SchemaArray {
    /// Type of array elements
    pub item_type: Box<Schema>,
    /// Length of array
    pub count: usize,
}

impl SchemaArray {
    fn serialized_size(&self) -> Option<usize> {
        self.item_type.serialized_size().map(|x| x * self.count)
    }
}

/// A struct is serialized by serializing its fields one by one,
/// without any padding.
/// The dbg_name is just for diagnostics.
#[derive(Debug, PartialEq)]
#[repr(C)]
pub struct SchemaStruct {
    /// Diagnostic value
    pub dbg_name: String,
    /// Fields of struct
    pub fields: Vec<Field>,
}
fn maybe_add(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    if let Some(a) = a {
        if let Some(b) = b {
            return Some(a + b);
        }
    }
    None
}
impl SchemaStruct {
    fn serialized_size(&self) -> Option<usize> {
        self.fields
            .iter()
            .fold(Some(0usize), |prev, x| maybe_add(prev, x.value.serialized_size()))
    }
}

/// An enum variant is serialized as its fields, one by one,
/// without any padding.
#[derive(Debug, PartialEq)]
pub struct Variant {
    /// Name of variant
    pub name: String,
    /// Discriminator in binary file-format
    pub discriminator: u8,
    /// Fields of variant
    pub fields: Vec<Field>,
}
impl Variant {
    fn serialized_size(&self) -> Option<usize> {
        self.fields
            .iter()
            .fold(Some(0usize), |prev, x| maybe_add(prev, x.value.serialized_size()))
    }
}

/// An enum is serialized as its u8 variant discriminator
/// followed by all the field for that variant.
/// The name of each variant, as well as its order in
/// the enum (the discriminator), is significant.
#[derive(Debug, PartialEq)]
pub struct SchemaEnum {
    /// Diagnostic name
    pub dbg_name: String,
    /// Variants of enum
    pub variants: Vec<Variant>,
}

fn maybe_max(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    if let Some(a) = a {
        if let Some(b) = b {
            return Some(a.max(b));
        }
    }
    None
}
impl SchemaEnum {
    fn serialized_size(&self) -> Option<usize> {
        let discr_size = 1usize; //Discriminator is always 1 byte
        self.variants
            .iter()
            .fold(Some(discr_size), |prev, x| maybe_max(prev, x.serialized_size()))
    }
}

/// A primitive is serialized as the little endian
/// representation of its type, except for string,
/// which is serialized as an usize length followed
/// by the string in utf8.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SchemaPrimitive {
    /// i8
    schema_i8,
    /// u8
    schema_u8,
    /// i16
    schema_i16,
    /// u16
    schema_u16,
    /// i32
    schema_i32,
    /// u32
    schema_u32,
    /// i64
    schema_i64,
    /// u64
    schema_u64,
    /// string
    schema_string,
    /// f32
    schema_f32,
    /// f64
    schema_f64,
    /// bool
    schema_bool,
    /// canary
    schema_canary1,
    /// u128
    schema_u128,
    /// i128
    schema_i128,
    /// char
    schema_char
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
            SchemaPrimitive::schema_u128 => "u128",
            SchemaPrimitive::schema_i128 => "i128",
            SchemaPrimitive::schema_char => "char",
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
            SchemaPrimitive::schema_i128|SchemaPrimitive::schema_u128 => Some(16),
            SchemaPrimitive::schema_char => {Some(4)}
        }
    }
}

fn diff_primitive(a: SchemaPrimitive, b: SchemaPrimitive, path: &str) -> Option<String> {
    if a != b {
        return Some(format!(
            "At location [{}]: Application protocol has datatype {}, but disk format has {}",
            path,
            a.name(),
            b.name()
        ));
    }
    None
}

/// The schema represents the save file format
/// of your data structure. It is an AST (Abstract Syntax Tree)
/// for consisting of various types of nodes in the savefile
/// format. Custom Serialize-implementations cannot add new types to
/// this tree, but must reuse these existing ones.
/// See the various enum variants for more information:
#[derive(Debug, PartialEq)]
#[repr(C,u32)]
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
    /// A user-defined, custom type. The string can be anything. The schema
    /// only matches if the string is identical
    Custom(String)
}

impl Schema {
    pub fn layout_compatible(&self, other: &Schema) -> bool {
        match (self, other) {
            (Schema::Struct(a),Schema::Struct(b)) => {
                a.layout_compatible(b)
            }
            (Schema::Enum(a), Schema::Enum(b)) => {
                a.layout_compatible(b)
            }
            (Schema::Primitive(a), Schema::Primitive(b)) => {
                a == b
            }
            (Schema::Vector(a), Schema::Vector(b)) => {
                a.layout_compatible(b)
            }
            (Schema::Array(a), Schema::Array(b)) => {
                a.layout_compatible(b)
            }
            (Schema::SchemaOption(a), Schema::SchemaOption(b)) => {
                a.layout_compatible(b)
            }
            (Schema::ZeroSize, Schema::ZeroSize) => {
                true
            }
            (Schema::Custom(a), Schema::Custom(b)) => {
                a == b
            }
            _ => false
        }
    }
    /// Create a 1-element tuple
    pub fn new_tuple1<T1: WithSchema>(version: u32) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "1-Tuple".to_string(),
            fields: vec![Field {
                name: "0".to_string(),
                value: Box::new(T1::schema(version)),
            }],
        })
    }

    /// Create a 2-element tuple
    pub fn new_tuple2<T1: WithSchema, T2: WithSchema>(version: u32) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "2-Tuple".to_string(),
            fields: vec![
                Field {
                    name: "0".to_string(),
                    value: Box::new(T1::schema(version)),
                },
                Field {
                    name: "1".to_string(),
                    value: Box::new(T2::schema(version)),
                },
            ],
        })
    }
    /// Create a 3-element tuple
    pub fn new_tuple3<T1: WithSchema, T2: WithSchema, T3: WithSchema>(version: u32) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "3-Tuple".to_string(),
            fields: vec![
                Field {
                    name: "0".to_string(),
                    value: Box::new(T1::schema(version)),
                },
                Field {
                    name: "1".to_string(),
                    value: Box::new(T2::schema(version)),
                },
                Field {
                    name: "2".to_string(),
                    value: Box::new(T3::schema(version)),
                },
            ],
        })
    }
    /// Create a 4-element tuple
    pub fn new_tuple4<T1: WithSchema, T2: WithSchema, T3: WithSchema, T4: WithSchema>(version: u32) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "4-Tuple".to_string(),
            fields: vec![
                Field {
                    name: "0".to_string(),
                    value: Box::new(T1::schema(version)),
                },
                Field {
                    name: "1".to_string(),
                    value: Box::new(T2::schema(version)),
                },
                Field {
                    name: "2".to_string(),
                    value: Box::new(T3::schema(version)),
                },
                Field {
                    name: "3".to_string(),
                    value: Box::new(T4::schema(version)),
                },
            ],
        })
    }
    /// Size
    pub fn serialized_size(&self) -> Option<usize> {
        match *self {
            Schema::Struct(ref schema_struct) => schema_struct.serialized_size(),
            Schema::Enum(ref schema_enum) => schema_enum.serialized_size(),
            Schema::Primitive(ref schema_primitive) => schema_primitive.serialized_size(),
            Schema::Vector(ref _vector) => None,
            Schema::Array(ref array) => array.serialized_size(),
            Schema::SchemaOption(ref _content) => None,
            Schema::Undefined => None,
            Schema::ZeroSize => Some(0),
            Schema::Custom(_) => None,
        }
    }
}

fn diff_vector(a: &Schema, b: &Schema, path: String) -> Option<String> {
    diff_schema(a, b, path + "/*")
}

fn diff_array(a: &SchemaArray, b: &SchemaArray, path: String) -> Option<String> {
    if a.count != b.count {
        return Some(format!(
            "At location [{}]: In memory array has length {}, but disk format length {}.",
            path, a.count, b.count
        ));
    }

    diff_schema(&a.item_type, &b.item_type, format!("{}/[{}]", path, a.count))
}

fn diff_option(a: &Schema, b: &Schema, path: String) -> Option<String> {
    diff_schema(a, b, path + "/?")
}

fn diff_enum(a: &SchemaEnum, b: &SchemaEnum, path: String) -> Option<String> {
    let path = (path + &b.dbg_name).to_string();
    if a.variants.len() != b.variants.len() {
        return Some(format!(
            "At location [{}]: In memory enum has {} variants, but disk format has {} variants.",
            path,
            a.variants.len(),
            b.variants.len()
        ));
    }
    for i in 0..a.variants.len() {
        if a.variants[i].name != b.variants[i].name {
            return Some(format!(
                "At location [{}]: Enum variant #{} in memory is called {}, but in disk format it is called {}",
                &path, i, a.variants[i].name, b.variants[i].name
            ));
        }
        if a.variants[i].discriminator != b.variants[i].discriminator {
            return Some(format!(
                "At location [{}]: Enum variant #{} in memory has discriminator {}, but in disk format it has {}",
                &path, i, a.variants[i].discriminator, b.variants[i].discriminator
            ));
        }
        let r = diff_fields(
            &a.variants[i].fields,
            &b.variants[i].fields,
            &(path.to_string() + "/" + &b.variants[i].name).to_string(),
            "enum",
            "",
            "",
        );
        if let Some(err) = r {
            return Some(err);
        }
    }
    None
}
fn diff_struct(a: &SchemaStruct, b: &SchemaStruct, path: String) -> Option<String> {
    diff_fields(
        &a.fields,
        &b.fields,
        &(path + "/" + &b.dbg_name).to_string(),
        "struct",
        &(" (struct ".to_string() + &a.dbg_name + ")"),
        &(" (struct ".to_string() + &b.dbg_name + ")"),
    )
}
fn diff_fields(
    a: &[Field],
    b: &[Field],
    path: &str,
    structuretype: &str,
    extra_a: &str,
    extra_b: &str,
) -> Option<String> {
    if a.len() != b.len() {
        return Some(format!(
            "At location [{}]: In memory {}{} has {} fields, disk format{} has {} fields.",
            path,
            structuretype,
            extra_a,
            a.len(),
            extra_b,
            b.len()
        ));
    }
    for i in 0..a.len() {
        let r = diff_schema(
            &a[i].value,
            &b[i].value,
            (path.to_string() + "/" + &b[i].name).to_string(),
        );
        if let Some(err) = r {
            return Some(err);
        }
    }
    None
}
/// Return a (kind of) human-readable description of the difference
/// between the two schemas. The schema 'a' is assumed to be the current
/// schema (used in memory).
/// Returns None if both schemas are equivalent
pub fn diff_schema(a: &Schema, b: &Schema, path: String) -> Option<String> {
    let (atype, btype) = match *a {
        Schema::Struct(ref xa) => match *b {
            Schema::Struct(ref xb) => return diff_struct(xa, xb, path),
            Schema::Enum(_) => ("struct", "enum"),
            Schema::Primitive(_) => ("struct", "primitive"),
            Schema::Vector(_) => ("struct", "vector"),
            Schema::SchemaOption(_) => ("struct", "option"),
            Schema::Undefined => ("struct", "undefined"),
            Schema::ZeroSize => ("struct", "zerosize"),
            Schema::Array(_) => ("struct", "array"),
            Schema::Custom(_) => ("struct", "custom"),
        },
        Schema::Enum(ref xa) => match *b {
            Schema::Enum(ref xb) => return diff_enum(xa, xb, path),
            Schema::Struct(_) => ("enum", "struct"),
            Schema::Primitive(_) => ("enum", "primitive"),
            Schema::Vector(_) => ("enum", "vector"),
            Schema::SchemaOption(_) => ("enum", "option"),
            Schema::Undefined => ("enum", "undefined"),
            Schema::ZeroSize => ("enum", "zerosize"),
            Schema::Array(_) => ("enum", "array"),
            Schema::Custom(_) => ("enum", "custom"),
        },
        Schema::Primitive(ref xa) => match *b {
            Schema::Primitive(ref xb) => {
                return diff_primitive(*xa, *xb, &path);
            }
            Schema::Struct(_) => ("primitive", "struct"),
            Schema::Enum(_) => ("primitive", "enum"),
            Schema::Vector(_) => ("primitive", "vector"),
            Schema::SchemaOption(_) => ("primitive", "option"),
            Schema::Undefined => ("primitive", "undefined"),
            Schema::ZeroSize => ("primitive", "zerosize"),
            Schema::Array(_) => ("primitive", "array"),
            Schema::Custom(_) => ("primitive", "custom"),
        },
        Schema::SchemaOption(ref xa) => match *b {
            Schema::SchemaOption(ref xb) => {
                return diff_option(xa, xb, path);
            }
            Schema::Struct(_) => ("option", "struct"),
            Schema::Enum(_) => ("option", "enum"),
            Schema::Primitive(_) => ("option", "primitive"),
            Schema::Vector(_) => ("option", "vector"),
            Schema::Undefined => ("option", "undefined"),
            Schema::ZeroSize => ("option", "zerosize"),
            Schema::Array(_) => ("option", "array"),
            Schema::Custom(_) => ("option", "custom"),
        },
        Schema::Vector(ref xa) => match *b {
            Schema::Vector(ref xb) => {
                return diff_vector(xa, xb, path);
            }
            Schema::Struct(_) => ("vector", "struct"),
            Schema::Enum(_) => ("vector", "enum"),
            Schema::Primitive(_) => ("vector", "primitive"),
            Schema::SchemaOption(_) => ("vector", "option"),
            Schema::Undefined => ("vector", "undefined"),
            Schema::ZeroSize => ("vector", "zerosize"),
            Schema::Array(_) => ("vector", "array"),
            Schema::Custom(_) => ("vector", "custom"),
        },
        Schema::Undefined => {
            return Some(format!("At location [{}]: Undefined schema encountered.", path));
        }
        Schema::ZeroSize => match *b {
            Schema::ZeroSize => {
                return None;
            }
            Schema::Vector(_) => ("zerosize", "vector"),
            Schema::Struct(_) => ("zerosize", "struct"),
            Schema::Enum(_) => ("zerosize", "enum"),
            Schema::SchemaOption(_) => ("zerosize", "option"),
            Schema::Primitive(_) => ("zerosize", "primitive"),
            Schema::Undefined => ("zerosize", "undefined"),
            Schema::Array(_) => ("zerosize", "array"),
            Schema::Custom(_) => ("zerosize", "custom"),

        },
        Schema::Array(ref xa) => match *b {
            Schema::Vector(_) => ("array", "vector"),
            Schema::Struct(_) => ("array", "struct"),
            Schema::Enum(_) => ("array", "enum"),
            Schema::Primitive(_) => ("array", "primitive"),
            Schema::SchemaOption(_) => ("array", "option"),
            Schema::Undefined => ("array", "undefined"),
            Schema::ZeroSize => ("array", "zerosize"),
            Schema::Array(ref xb) => return diff_array(xa, xb, path),
            Schema::Custom(_) => ("array", "custom"),
        },
        Schema::Custom(ref custom_a) => match b {
            Schema::Vector(_) => ("custom", "vector"),
            Schema::Struct(_) => ("custom", "struct"),
            Schema::Enum(_) => ("custom", "enum"),
            Schema::Primitive(_) => ("custom", "primitive"),
            Schema::SchemaOption(_) => ("custom", "option"),
            Schema::Undefined => ("custom", "undefined"),
            Schema::ZeroSize => ("custom", "zerosize"),
            Schema::Array(_) => ("custom", "array"),
            Schema::Custom(custom_b) => {
                if a != b {
                    return Some(format!(
                        "At location [{}]: Application protocol has datatype Custom({}), but disk format has Custom({})",
                        path,
                        custom_a,
                        custom_b
                    ));
                }
                return None;
            }
        }
    };
    Some(format!(
        "At location [{}]: In memory schema: {}, file schema: {}",
        path, atype, btype
    ))
}

impl WithSchema for Field {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}

impl Serialize for Field {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.name)?;
        self.value.serialize(serializer)
    }
}
impl ReprC for Field {}
impl Deserialize for Field {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Field {
            name: deserializer.read_string()?,
            value: Box::new(Schema::deserialize(deserializer)?),
        })
    }
}
impl WithSchema for Variant {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for Variant {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.name)?;
        serializer.write_u8(self.discriminator)?;
        serializer.write_usize(self.fields.len())?;
        for field in &self.fields {
            field.serialize(serializer)?;
        }
        Ok(())
    }
}

impl ReprC for Variant {}
impl Deserialize for Variant {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Variant {
            name: deserializer.read_string()?,
            discriminator: deserializer.read_u8()?,
            fields: {
                let l = deserializer.read_usize()?;
                let mut ret = Vec::new();
                for _ in 0..l {
                    ret.push(Field {
                        name: deserializer.read_string()?,
                        value: Box::new(Schema::deserialize(deserializer)?),
                    });
                }
                ret
            },
        })
    }
}
impl Serialize for SchemaArray {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(self.count)?;
        self.item_type.serialize(serializer)?;
        Ok(())
    }
}
impl ReprC for SchemaArray {}
impl Deserialize for SchemaArray {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let count = deserializer.read_usize()?;
        let item_type = Box::new(Schema::deserialize(deserializer)?);
        Ok(SchemaArray { count, item_type })
    }
}
impl WithSchema for SchemaArray {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}

impl WithSchema for SchemaStruct {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for SchemaStruct {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.dbg_name)?;
        serializer.write_usize(self.fields.len())?;
        for field in &self.fields {
            field.serialize(serializer)?;
        }
        Ok(())
    }
}
impl ReprC for SchemaStruct {}
impl Deserialize for SchemaStruct {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let dbg_name = deserializer.read_string()?;
        let l = deserializer.read_usize()?;
        Ok(SchemaStruct {
            dbg_name,
            fields: {
                let mut ret = Vec::new();
                for _ in 0..l {
                    ret.push(Field::deserialize(deserializer)?)
                }
                ret
            },
        })
    }
}

impl WithSchema for SchemaPrimitive {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for SchemaPrimitive {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let discr = match *self {
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
            SchemaPrimitive::schema_i128 => 14,
            SchemaPrimitive::schema_u128 => 15,
            SchemaPrimitive::schema_char => 16,
        };
        serializer.write_u8(discr)
    }
}
impl ReprC for SchemaPrimitive {}
impl Deserialize for SchemaPrimitive {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let var = match deserializer.read_u8()? {
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
            14 => SchemaPrimitive::schema_i128,
            15 => SchemaPrimitive::schema_u128,
            16 => SchemaPrimitive::schema_char,
            c => {
                return Err(SavefileError::GeneralError {
                    msg: format!("Corrupt schema, type {} encountered. Perhaps data is from future version?", c),
                })
            }
        };
        Ok(var)
    }
}

impl WithSchema for SchemaEnum {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}

impl Serialize for SchemaEnum {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.dbg_name)?;
        serializer.write_usize(self.variants.len())?;
        for var in &self.variants {
            var.serialize(serializer)?;
        }
        Ok(())
    }
}
impl ReprC for SchemaEnum {}
impl Deserialize for SchemaEnum {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let dbg_name = deserializer.read_string()?;
        let l = deserializer.read_usize()?;
        let mut ret = Vec::new();
        for _ in 0..l {
            ret.push(Variant::deserialize(deserializer)?);
        }
        Ok(SchemaEnum {
            dbg_name,
            variants: ret,
        })
    }
}

impl WithSchema for Schema {
    fn schema(_version: u32) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for Schema {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        match *self {
            Schema::Struct(ref schema_struct) => {
                serializer.write_u8(1)?;
                schema_struct.serialize(serializer)
            }
            Schema::Enum(ref schema_enum) => {
                serializer.write_u8(2)?;
                schema_enum.serialize(serializer)
            }
            Schema::Primitive(ref schema_prim) => {
                serializer.write_u8(3)?;
                schema_prim.serialize(serializer)
            }
            Schema::Vector(ref schema_vector) => {
                serializer.write_u8(4)?;
                schema_vector.serialize(serializer)
            }
            Schema::Undefined => serializer.write_u8(5),
            Schema::ZeroSize => serializer.write_u8(6),
            Schema::SchemaOption(ref content) => {
                serializer.write_u8(7)?;
                content.serialize(serializer)
            }
            Schema::Array(ref array) => {
                serializer.write_u8(8)?;
                array.serialize(serializer)
            }
            Schema::Custom(ref custom) => {
                serializer.write_u8(9)?;
                custom.serialize(serializer)
            }
        }
    }
}

impl ReprC for Schema {}
impl Deserialize for Schema {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let schema = match deserializer.read_u8()? {
            1 => Schema::Struct(SchemaStruct::deserialize(deserializer)?),
            2 => Schema::Enum(SchemaEnum::deserialize(deserializer)?),
            3 => Schema::Primitive(SchemaPrimitive::deserialize(deserializer)?),
            4 => Schema::Vector(Box::new(Schema::deserialize(deserializer)?)),
            5 => Schema::Undefined,
            6 => Schema::ZeroSize,
            7 => Schema::SchemaOption(Box::new(Schema::deserialize(deserializer)?)),
            8 => Schema::Array(SchemaArray::deserialize(deserializer)?),
            9 => Schema::Custom(String::deserialize(deserializer)?),
            c => {
                return Err(SavefileError::GeneralError {
                    msg: format!("Corrupt schema, schema variant {} encountered", c),
                })
            }
        };

        Ok(schema)
    }
}

impl WithSchema for String {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }
}

impl Introspect for String {
    fn introspect_value(&self) -> String {
        self.to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem>> {
        None
    }
}
impl Serialize for String {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(self)
    }
}

impl ReprC for String {}

impl Deserialize for String {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<String, SavefileError> {
        deserializer.read_string()
    }
}

/// Type of single child of introspector for Mutex
#[cfg(feature="parking_lot")]
pub struct IntrospectItemMutex<'a, T> {
    g: MutexGuard<'a, T>,
}

#[cfg(feature="parking_lot")]
impl<'a, T: Introspect> IntrospectItem<'a> for IntrospectItemMutex<'a, T> {
    fn key(&self) -> &str {
        "0"
    }

    fn val(&self) -> &dyn Introspect {
        self.g.deref()
    }
}

#[cfg(feature="parking_lot")]
impl<T: Introspect> Introspect for Mutex<T> {
    fn introspect_value(&self) -> String {
        format!("Mutex<{}>", std::any::type_name::<T>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            Some(Box::new(IntrospectItemMutex { g: self.lock() }))
        } else {
            None
        }
    }
}

/// Type of single child of introspector for std::sync::Mutex
pub struct IntrospectItemStdMutex<'a, T> {
    g: std::sync::MutexGuard<'a, T>,
}

impl<'a, T: Introspect> IntrospectItem<'a> for IntrospectItemStdMutex<'a, T> {
    fn key(&self) -> &str {
        "0"
    }

    fn val(&self) -> &dyn Introspect {
        self.g.deref()
    }
}

impl<T: Introspect> Introspect for std::sync::Mutex<T> {
    fn introspect_value(&self) -> String {
        format!("Mutex<{}>", std::any::type_name::<T>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        match self.lock() {
            Ok(item) => {
                if index == 0 {
                    Some(Box::new(IntrospectItemStdMutex { g: item }))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}

impl<T: WithSchema> WithSchema for std::sync::Mutex<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<T> ReprC for std::sync::Mutex<T> {}
impl<T: Serialize> Serialize for std::sync::Mutex<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let data = self.lock()?;
        data.serialize(serializer)
    }
}

impl<T: Deserialize> Deserialize for std::sync::Mutex<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<std::sync::Mutex<T>, SavefileError> {
        Ok(std::sync::Mutex::new(T::deserialize(deserializer)?))
    }
}

#[cfg(feature="parking_lot")]
impl<T: WithSchema> WithSchema for Mutex<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}

#[cfg(feature="parking_lot")]
impl<T> ReprC for Mutex<T> {}

#[cfg(feature="parking_lot")]
impl<T: Serialize> Serialize for Mutex<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let data = self.lock();
        data.serialize(serializer)
    }
}

#[cfg(feature="parking_lot")]
impl<T: Deserialize> Deserialize for Mutex<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Mutex<T>, SavefileError> {
        Ok(Mutex::new(T::deserialize(deserializer)?))
    }
}

/// Type of single child of introspector for RwLock
#[cfg(feature="parking_lot")]
pub struct IntrospectItemRwLock<'a, T> {
    g: RwLockReadGuard<'a, T>,
}

#[cfg(feature="parking_lot")]
impl<'a, T: Introspect> IntrospectItem<'a> for IntrospectItemRwLock<'a, T> {
    fn key(&self) -> &str {
        "0"
    }

    fn val(&self) -> &dyn Introspect {
        self.g.deref()
    }
}

impl<'a,T:Introspect> Introspect for std::cell::Ref<'a,T> {
    fn introspect_value(&self) -> String {
        let sub_value = (**self).introspect_value();
        format!("Ref({})", sub_value)
    }
    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        (**self).introspect_child(index)
    }
    fn introspect_len(&self) -> usize {
        (**self).introspect_len()
    }
}

impl<'a,T:Introspect> IntrospectItem<'a> for std::cell::Ref<'a,T> {
    fn key(&self) -> &str {
        "ref"
    }
    /// The introspectable value of the child.
    fn val(&self) -> &dyn Introspect {
        &*self
    }
}

impl<T: Introspect> Introspect for RefCell<T> {
    fn introspect_value(&self) -> String {
        let sub_value = self.borrow().introspect_value();
        format!("RefCell({})", sub_value)
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        // Introspect not supported
        if index != 0 {
            return None;
        }
        let rf = self.borrow();
        Some(Box::new(rf))
    }

    fn introspect_len(&self) -> usize {
        // Introspect not supported
        1
    }
}

impl<T: Introspect> Introspect for Rc<T> {
    fn introspect_value(&self) -> String {
        format!("Rc({})", self.deref().introspect_value())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        self.deref().introspect_child(index)
    }

    fn introspect_len(&self) -> usize {
        self.deref().introspect_len()
    }
}

impl<T: Introspect> Introspect for Arc<T> {
    fn introspect_value(&self) -> String {
        format!("Arc({})", self.deref().introspect_value())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        self.deref().introspect_child(index)
    }

    fn introspect_len(&self) -> usize {
        self.deref().introspect_len()
    }
}
#[cfg(feature="parking_lot")]
impl<T: Introspect> Introspect for RwLock<T> {
    fn introspect_value(&self) -> String {
        format!("RwLock<{}>", std::any::type_name::<T>())
    }
    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            Some(Box::new(IntrospectItemRwLock { g: self.read() }))
        } else {
            None
        }
    }

    fn introspect_len(&self) -> usize {
        1
    }
}

#[cfg(feature="parking_lot")]
impl<T: WithSchema> WithSchema for RwLock<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}

#[cfg(feature="parking_lot")]
impl<T> ReprC for RwLock<T> {}

#[cfg(feature="parking_lot")]
impl<T: Serialize> Serialize for RwLock<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let data = self.read();
        data.serialize(serializer)
    }
}

#[cfg(feature="parking_lot")]
impl<T: Deserialize> Deserialize for RwLock<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<RwLock<T>, SavefileError> {
        Ok(RwLock::new(T::deserialize(deserializer)?))
    }
}

/// Standard child for Introspect trait. Simply owned key string and reference to dyn Introspect
pub struct IntrospectItemSimple<'a> {
    key: String,
    val: &'a dyn Introspect,
}

impl<'a> IntrospectItem<'a> for IntrospectItemSimple<'a> {
    fn key(&self) -> &str {
        &self.key
    }

    fn val(&self) -> &dyn Introspect {
        self.val
    }
}

/// Create a default IntrospectItem with the given key and Introspect.
pub fn introspect_item<'a>(key: String, val: &'a dyn Introspect) -> Box<dyn IntrospectItem<'a> + 'a> {
    Box::new(IntrospectItemSimple { key: key, val: val })
}



#[cfg(not(feature = "nightly"))]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for HashMap<K, V, S> {
    fn introspect_value(&self) -> String {
        format!("HashMap<{},{}>", std::any::type_name::<K>(), std::any::type_name::<V>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((key, val)) = self.iter().skip(bucket).next() {
            if off == 0 {
                Some(introspect_item(format!("Key #{}", index), key))
            } else {
                Some(introspect_item(format!("Value #{}", index), val))
            }
        } else {
            None
        }
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(feature = "nightly")]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for HashMap<K, V, S> {
    default fn introspect_value(&self) -> String {
        format!("HashMap<{},{}>", std::any::type_name::<K>(), std::any::type_name::<V>())
    }

    default fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((key, val)) = self.iter().skip(bucket).next() {
            if off == 0 {
                Some(introspect_item(format!("Key #{}", index), key))
            } else {
                Some(introspect_item(format!("Value #{}", index), val))
            }
        } else {
            None
        }
    }
    default fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(feature = "nightly")]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for HashMap<K, V, S>
where
    K: ToString,
{
    fn introspect_value(&self) -> String {
        format!("HashMap<{},{}>", std::any::type_name::<K>(), std::any::type_name::<V>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some((key, val)) = self.iter().skip(index).next() {
            Some(introspect_item(key.to_string(), val))
        } else {
            None
        }
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<K: Introspect + Eq + Hash, S: ::std::hash::BuildHasher> Introspect for HashSet<K, S> {
    fn introspect_value(&self) -> String {
        format!("HashSet<{}>", std::any::type_name::<K>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some(key) = self.iter().skip(index).next() {
            Some(introspect_item(format!("#{}", index), key))
        } else {
            None
        }
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<K: Introspect, V: Introspect> Introspect for BTreeMap<K, V> {
    fn introspect_value(&self) -> String {
        format!("BTreeMap<{},{}>", std::any::type_name::<K>(), std::any::type_name::<V>())
    }

    // This has very bad performance. But with the model behind Savefile Introspect it
    // is presently hard to do much better
    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((key, val)) = self.iter().skip(bucket).next() {
            if off == 0 {
                Some(introspect_item(format!("Key #{}", index), key))
            } else {
                Some(introspect_item(format!("Value #{}", index), val))
            }
        } else {
            None
        }
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}
impl<K: WithSchema, V: WithSchema> WithSchema for BTreeMap<K, V> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(Schema::Struct(SchemaStruct {
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
            ],
        })))
    }
}
impl<K, V> ReprC for BTreeMap<K, V> {}
impl<K: Serialize, V: Serialize> Serialize for BTreeMap<K, V> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.len().serialize(serializer)?;
        for (k, v) in self {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}
impl<K: Deserialize + Ord, V: Deserialize> Deserialize for BTreeMap<K, V> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let mut ret = BTreeMap::new();
        let count = <usize as Deserialize>::deserialize(deserializer)?;
        for _ in 0..count {
            ret.insert(
                <_ as Deserialize>::deserialize(deserializer)?,
                <_ as Deserialize>::deserialize(deserializer)?,
            );
        }
        Ok(ret)
    }
}

















impl<K, S: ::std::hash::BuildHasher> ReprC for HashSet<K,S> {}
impl<K:WithSchema, S: ::std::hash::BuildHasher> WithSchema for HashSet<K,S> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(K::schema(version)))
    }
}
impl<K:Serialize, S: ::std::hash::BuildHasher> Serialize for HashSet<K,S> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(self.len())?;
        for item in self {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}
impl<K:Deserialize+Eq+Hash, S: ::std::hash::BuildHasher+Default> Deserialize for HashSet<K,S> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let cnt = deserializer.read_usize()?;
        let mut ret = HashSet::with_capacity_and_hasher(cnt, S::default());
        for _ in 0..cnt {
            ret.insert(<_ as Deserialize>::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

impl<K: WithSchema + Eq + Hash, V: WithSchema, S: ::std::hash::BuildHasher> WithSchema for HashMap<K, V, S> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(Schema::Struct(SchemaStruct {
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
            ],
        })))
    }
}
impl<K:Eq + Hash, V, S: ::std::hash::BuildHasher> ReprC for HashMap<K, V, S> {}
impl<K: Serialize + Eq + Hash, V: Serialize, S: ::std::hash::BuildHasher> Serialize for HashMap<K, V, S> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(self.len())?;
        for (k, v) in self.iter() {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<K: Deserialize + Eq + Hash, V: Deserialize, S: ::std::hash::BuildHasher+Default> Deserialize for HashMap<K, V, S> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret:Self = HashMap::with_capacity_and_hasher(l, Default::default());
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

#[cfg(feature="indexmap")]
impl<K: WithSchema + Eq + Hash, V: WithSchema, S: ::std::hash::BuildHasher> WithSchema for IndexMap<K, V, S> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(Schema::Struct(SchemaStruct {
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
            ],
        })))
    }
}

#[cfg(all(not(feature = "nightly"), feature="indexmap"))]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for IndexMap<K, V, S> {
    fn introspect_value(&self) -> String {
        format!(
            "IndexMap<{},{}>",
            std::any::type_name::<K>(),
            std::any::type_name::<V>()
        )
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((k, v)) = self.get_index(bucket) {
            if off == 0 {
                Some(introspect_item(format!("Key #{}", bucket), k))
            } else {
                Some(introspect_item(format!("Value #{}", bucket), v))
            }
        } else {
            None
        }
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(all(feature = "nightly", feature="indexmap"))]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for IndexMap<K, V, S> {
    default fn introspect_value(&self) -> String {
        format!(
            "IndexMap<{},{}>",
            std::any::type_name::<K>(),
            std::any::type_name::<V>()
        )
    }

    default fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((k, v)) = self.get_index(bucket) {
            if off == 0 {
                Some(introspect_item(format!("Key #{}", bucket), k))
            } else {
                Some(introspect_item(format!("Value #{}", bucket), v))
            }
        } else {
            None
        }
    }

    default fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(all(feature = "nightly", feature="indexmap"))]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for IndexMap<K, V, S>
where
    K: ToString,
{
    fn introspect_value(&self) -> String {
        format!(
            "IndexMap<{},{}>",
            std::any::type_name::<K>(),
            std::any::type_name::<V>()
        )
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some((k, v)) = self.get_index(index) {
            Some(introspect_item(k.to_string(), v))
        } else {
            None
        }
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}
#[cfg(feature="indexmap")]
impl<K: Eq + Hash, V, S: ::std::hash::BuildHasher> ReprC for IndexMap<K, V, S> {}

#[cfg(feature="indexmap")]
impl<K: Serialize + Eq + Hash, V: Serialize, S: ::std::hash::BuildHasher> Serialize for IndexMap<K, V, S> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(self.len())?;
        for (k, v) in self.iter() {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}

#[cfg(feature="indexmap")]
impl<K: Deserialize + Eq + Hash, V: Deserialize> Deserialize for IndexMap<K, V> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = IndexMap::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

#[cfg(feature="indexmap")]
impl<K: Introspect + Eq + Hash, S: ::std::hash::BuildHasher> Introspect for IndexSet<K, S> {
    fn introspect_value(&self) -> String {
        format!("IndexSet<{}>", std::any::type_name::<K>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some(val) = self.get_index(index) {
            Some(introspect_item(format!("#{}", index), val))
        } else {
            None
        }
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(feature="indexmap")]
impl<K:Eq + Hash, S: ::std::hash::BuildHasher> ReprC for IndexSet<K, S> {}

#[cfg(feature="indexmap")]
impl<K: WithSchema + Eq + Hash, S: ::std::hash::BuildHasher> WithSchema for IndexSet<K, S> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(Schema::Struct(SchemaStruct {
            dbg_name: "Key".to_string(),
            fields: vec![Field {
                name: "key".to_string(),
                value: Box::new(K::schema(version)),
            }],
        })))
    }
}

#[cfg(feature="indexmap")]
impl<K: Serialize + Eq + Hash, S: ::std::hash::BuildHasher> Serialize for IndexSet<K, S> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(self.len())?;
        for k in self.iter() {
            k.serialize(serializer)?;
        }
        Ok(())
    }
}

#[cfg(feature="indexmap")]
impl<K: Deserialize + Eq + Hash> Deserialize for IndexSet<K> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = IndexSet::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

/// Helper struct which represents a field which has been removed
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Hash,Default)]
pub struct Removed<T> {
    phantom: std::marker::PhantomData<*const T>,
}

/// Removed is a zero-sized type. It contains a PhantomData<*const T>, which means
/// it doesn't implement Send or Sync per default. However, implementing these
/// is actually safe, so implement it manually.
unsafe impl<T> Send for Removed<T> {
}
/// Removed is a zero-sized type. It contains a PhantomData<*const T>, which means
/// it doesn't implement Send or Sync per default. However, implementing these
/// is actually safe, so implement it manually.
unsafe impl<T> Sync for Removed<T> {
}

impl<T> Removed<T> {
    /// Helper to create an instance of Removed<T>. Removed<T> has no data.
    pub fn new() -> Removed<T> {
        Removed {
            phantom: std::marker::PhantomData,
        }
    }
}
impl<T: WithSchema> WithSchema for Removed<T> {
    fn schema(version: u32) -> Schema {
        <T>::schema(version)
    }
}

impl<T: Introspect> Introspect for Removed<T> {
    fn introspect_value(&self) -> String {
        format!("Removed<{}>", std::any::type_name::<T>())
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl<T> ReprC for Removed<T> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl<T: WithSchema> Serialize for Removed<T> {
    fn serialize(&self, _serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        panic!("Something is wrong with version-specification of fields - there was an attempt to actually serialize a removed field!");
    }
}
impl<T: WithSchema + Deserialize> Deserialize for Removed<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        T::deserialize(deserializer)?;
        Ok(Removed {
            phantom: std::marker::PhantomData,
        })
    }
}

impl<T> Introspect for PhantomData<T> {
    fn introspect_value(&self) -> String {
        "PhantomData".to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl<T> WithSchema for std::marker::PhantomData<T> {
    fn schema(_version: u32) -> Schema {
        Schema::ZeroSize
    }
}
impl<T> ReprC for std::marker::PhantomData<T> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl<T> Serialize for std::marker::PhantomData<T> {
    fn serialize(&self, _serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        Ok(())
    }
}
impl<T> Deserialize for std::marker::PhantomData<T> {
    fn deserialize(_deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(std::marker::PhantomData)
    }
}

impl<T: Introspect> Introspect for Box<T> {
    fn introspect_value(&self) -> String {
        self.deref().introspect_value()
    }
    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        self.deref().introspect_child(index)
    }
    fn introspect_len(&self) -> usize {
        self.deref().introspect_len()
    }
}
impl<T: Introspect> Introspect for Option<T> {
    fn introspect_value(&self) -> String {
        if let Some(cont) = self {
            format!("Some({})", cont.introspect_value())
        } else {
            "None".to_string()
        }
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some(cont) = self {
            cont.introspect_child(index)
        } else {
            None
        }
    }
    fn introspect_len(&self) -> usize {
        if let Some(cont) = self {
            cont.introspect_len()
        } else {
            0
        }
    }
}

impl<T: WithSchema> WithSchema for Option<T> {
    fn schema(version: u32) -> Schema {
        Schema::SchemaOption(Box::new(T::schema(version)))
    }
}
impl<T> ReprC for Option<T> { } //Sadly, Option does not allow the #"reprC"-optimization
impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        match self {
            &Some(ref x) => {
                serializer.write_bool(true)?;
                x.serialize(serializer)
            }
            &None => serializer.write_bool(false),
        }
    }
}
impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let issome = deserializer.read_bool()?;
        if issome {
            Ok(Some(T::deserialize(deserializer)?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(feature="bit-vec")]
#[cfg(target_endian="big")]
compile_error!("savefile bit-vec feature does not support big-endian machines");

#[cfg(feature="bit-vec")]
impl WithSchema for bit_vec::BitVec {
    fn schema(version: u32) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "BitVec".to_string(),
            fields: vec![
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
                    value: Box::new(Schema::Vector(Box::new(u8::schema(version)))),
                },
            ],
        })
    }
}

#[cfg(feature="bit-vec")]
impl Introspect for bit_vec::BitVec {
    fn introspect_value(&self) -> String {
        let mut ret = String::new();
        for i in 0..self.len() {
            if self[i] {
                ret.push('1');
            } else {
                ret.push('0');
            }
        }
        ret
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

#[cfg(feature="bit-vec")]
impl Serialize for bit_vec::BitVec<u32> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize(l)?;
        let storage = self.storage();
        let rawbytes_ptr = storage.as_ptr() as *const u8;
        let rawbytes :&[u8] = unsafe{std::slice::from_raw_parts(rawbytes_ptr,4*storage.len())};
        serializer.write_usize(rawbytes.len()|(1<<63))?;
        serializer.write_bytes(&rawbytes)?;
        Ok(())
    }
}

#[cfg(feature="bit-vec")]
impl ReprC for bit_vec::BitVec<u32> {}

#[cfg(feature="bit-vec")]
impl Deserialize for bit_vec::BitVec<u32> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {

        let numbits = deserializer.read_usize()?;
        let mut numbytes = deserializer.read_usize()?;
        if numbytes&(1<<63)!=0 {
            //New format
            numbytes &= !(1<<63);
            let mut ret = bit_vec::BitVec::with_capacity(numbytes*8);
            unsafe {
                let num_words = numbytes/4;
                let storage = ret.storage_mut();
                storage.resize(num_words, 0);
                let storage_ptr = storage.as_ptr() as *mut u8;
                let storage_bytes:&mut [u8] = std::slice::from_raw_parts_mut(storage_ptr,4*num_words);
                deserializer.read_bytes_to_buf(storage_bytes)?;
                ret.set_len(numbits);
            }
            Ok(ret)
        } else {
            let bytes = deserializer.read_bytes(numbytes)?;
            let mut ret = bit_vec::BitVec::from_bytes(&bytes);
            ret.truncate(numbits);
            Ok(ret)
        }

    }
}


#[cfg(feature="bit-set")]
impl WithSchema for bit_set::BitSet {
    fn schema(version: u32) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "BitSet".to_string(),
            fields: vec![
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
                    value: Box::new(Schema::Vector(Box::new(u8::schema(version)))),
                },
            ],
        })
    }
}


#[cfg(feature="bit-set")]
impl Introspect for bit_set::BitSet {
    fn introspect_value(&self) -> String {
        let mut ret = String::new();
        for i in 0..self.len() {
            if self.contains(i) {
                use std::fmt::Write;
                write!(&mut ret, "{} ",i).unwrap();
            }
        }
        ret
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

#[cfg(feature="bit-set")]
impl ReprC for bit_set::BitSet<u32> {}

#[cfg(feature="bit-set")]
impl Serialize for bit_set::BitSet<u32> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let bitset = self.get_ref();
        bitset.serialize(serializer)
    }
}

#[cfg(feature="bit-set")]
impl Deserialize for bit_set::BitSet<u32> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let bit_vec: bit_vec::BitVec = bit_vec::BitVec::deserialize(deserializer)?;
        Ok(bit_set::BitSet::from_bit_vec(bit_vec))
    }
}

impl<T: Introspect> Introspect for BinaryHeap<T> {
    fn introspect_value(&self) -> String {
        "BinaryHeap".to_string()
    }

    fn introspect_child<'a>(&'a self, index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        if index >= self.len() {
            return None;
        }
        return Some(introspect_item(
            index.to_string(),
            self.iter().skip(index).next().unwrap(),
        ));
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<T> ReprC for BinaryHeap<T> {}
impl<T: WithSchema> WithSchema for BinaryHeap<T> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}
impl<T: Serialize + Ord> Serialize for BinaryHeap<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize(l)?;
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())
    }
}
impl<T: Deserialize + Ord> Deserialize for BinaryHeap<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = BinaryHeap::with_capacity(l);
        for _ in 0..l {
            ret.push(T::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

#[cfg(feature="smallvec")]
impl<T: smallvec::Array> Introspect for smallvec::SmallVec<T>
where
    T::Item: Introspect,
{
    fn introspect_value(&self) -> String {
        format!("SmallVec<{}>", std::any::type_name::<T>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some(val) = self.get(index) {
            Some(introspect_item(index.to_string(), val))
        } else {
            None
        }
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(feature="smallvec")]
impl<T: smallvec::Array> WithSchema for smallvec::SmallVec<T>
where
    T::Item: WithSchema,
{
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(T::Item::schema(version)))
    }
}
#[cfg(feature="smallvec")]
impl<T: smallvec::Array> ReprC for smallvec::SmallVec<T>{}

#[cfg(feature="smallvec")]
impl<T: smallvec::Array> Serialize for smallvec::SmallVec<T>
where
    T::Item: Serialize,
{
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize(l)?;
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())
    }
}
#[cfg(feature="smallvec")]
impl<T: smallvec::Array> Deserialize for smallvec::SmallVec<T>
where
    T::Item: Deserialize,
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize()?;
        let mut ret = Self::with_capacity(l);
        for _ in 0..l {
            ret.push(T::Item::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

fn regular_serialize_vec<T: Serialize>(items: &[T], serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
    let l = items.len();
    serializer.write_usize(l)?;
    if std::mem::size_of::<T>() == 0 {
        return Ok(());
    }

    if std::mem::size_of::<T>() < 32 { //<-- This optimization seems to help a little actually, but maybe not enough to warrant it
        let chunks = items.chunks_exact((64/std::mem::size_of::<T>()).max(1));
        let remainder = chunks.remainder();
        for chunk in chunks {
            for item in chunk {
                item.serialize(serializer)?;
            }
        }
        for item in remainder {
            item.serialize(serializer)?;
        }
        Ok(())
    } else {
        for item in items {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<T: WithSchema> WithSchema for Box<[T]> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}
impl<T: WithSchema> WithSchema for Arc<[T]> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}
impl<T: Introspect> Introspect for Box<[T]> {
    fn introspect_value(&self) -> String {
        return "Box[]".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index >= self.len() {
            return None;
        }
        return Some(introspect_item(index.to_string(), &self[index]));
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<T: Introspect> Introspect for Arc<[T]> {
    fn introspect_value(&self) -> String {
        return "Arc[]".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index >= self.len() {
            return None;
        }
        return Some(introspect_item(index.to_string(), &self[index]));
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl WithSchema for Arc<str> {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }
}
impl Introspect for Arc<str> {
    fn introspect_value(&self) -> String {
        self.deref().to_string()
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a>>> {
        None
    }
    fn introspect_len(&self) -> usize {
        0
    }
}
impl Serialize for Arc<str> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&*self)
    }
}

impl ReprC for Arc<str> {}

impl Deserialize for Arc<str> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let s = deserializer.read_string()?;

        let state = deserializer.get_state::<Arc<str>, HashMap<String, Arc<str>>>();

        if let Some(needle) = state.get(&s) {
            return Ok(Arc::clone(needle));
        }

        let arc_ref = state.entry(s.clone()).or_insert(s.into());
        Ok(Arc::clone(arc_ref))
    }
}

impl<T: Serialize + ReprC> Serialize for Box<[T]> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.version).is_false() {
                regular_serialize_vec(&*self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    (*self).as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}
impl<T: ReprC> ReprC for Box<[T]> { }


impl<T: Serialize + ReprC> Serialize for Arc<[T]> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.version).is_false() {
                regular_serialize_vec(&*self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    (*self).as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}
impl<T: ReprC> ReprC for Arc<[T]> { }

impl<T: Deserialize+ReprC> Deserialize for Arc<[T]> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Vec::<T>::deserialize(deserializer)?.into())
    }
}
impl<T: Deserialize+ReprC> Deserialize for Box<[T]> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Vec::<T>::deserialize(deserializer)?.into_boxed_slice())
    }
}

impl<T> ReprC for Vec<T> {}

impl<T: WithSchema> WithSchema for Vec<T> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}

impl<T: Introspect> Introspect for Vec<T> {
    fn introspect_value(&self) -> String {
        return "vec[]".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index >= self.len() {
            return None;
        }
        return Some(introspect_item(index.to_string(), &self[index]));
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<T: Serialize + ReprC> Serialize for Vec<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.version).is_false() {
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

fn regular_deserialize_vec<T: Deserialize>(deserializer: &mut Deserializer<impl Read>) -> Result<Vec<T>, SavefileError> {
    let l = deserializer.read_usize()?;

    #[cfg(feature = "size_sanity_checks")]
    {
        if l > 1_000_000 {
            return Err(SavefileError::GeneralError {
                msg: format!("Too many items in Vec: {}", l),
            });
        }
    }
    let mut ret = Vec::with_capacity(l);
    for _ in 0..l {
        ret.push(T::deserialize(deserializer)?);
    }
    Ok(ret)
}

impl<T: Deserialize + ReprC> Deserialize for Vec<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        if unsafe{T::repr_c_optimization_safe(deserializer.file_version)}.is_false() {
            Ok(regular_deserialize_vec(deserializer)?)
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
            let ptr =
                if elem_size == 0 {
                    NonNull::dangling().as_ptr()
                } else {
                    let ptr = unsafe { std::alloc::alloc(layout.clone()) };
                    if ptr.is_null() {
                        panic!("Failed to allocate {} bytes of memory", num_bytes);
                    }

                    ptr
                };

            {

                let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, num_bytes) };
                match deserializer.reader.read_exact(slice) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        unsafe {
                            std::alloc::dealloc(ptr, layout);
                        }
                        Err(err)
                    }
                }?;
            }
            let ret = unsafe { Vec::from_raw_parts(ptr as *mut T, num_elems, num_elems) };
            Ok(ret)
        }
    }
}

impl<T: Introspect> Introspect for VecDeque<T> {
    fn introspect_value(&self) -> String {
        format!("VecDeque<{}>", std::any::type_name::<T>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some(val) = self.get(index) {
            Some(introspect_item(index.to_string(), val))
        } else {
            None
        }
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<T: WithSchema> WithSchema for VecDeque<T> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(T::schema(version)))
    }
}

impl<T> ReprC for VecDeque<T> {}
impl<T: Serialize> Serialize for VecDeque<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        regular_serialize_vecdeque(self, serializer)
    }
}

impl<T: Deserialize> Deserialize for VecDeque<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(regular_deserialize_vecdeque(deserializer)?)
    }
}

fn regular_serialize_vecdeque<T: Serialize>(
    item: &VecDeque<T>,
    serializer: &mut Serializer<impl Write>,
) -> Result<(), SavefileError> {
    let l = item.len();
    serializer.write_usize(l)?;
    for item in item.iter() {
        item.serialize(serializer)?
    }
    Ok(())
}

fn regular_deserialize_vecdeque<T: Deserialize>(deserializer: &mut Deserializer<impl Read>) -> Result<VecDeque<T>, SavefileError> {
    let l = deserializer.read_usize()?;
    let mut ret = VecDeque::with_capacity(l);
    for _ in 0..l {
        ret.push_back(T::deserialize(deserializer)?);
    }
    Ok(ret)
}

impl ReprC for bool {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
} //It isn't really guaranteed that bool is an u8 or i8 where false = 0 and true = 1. But it's true in practice. And the breakage would be hard to measure if this were ever changed, so a change is unlikely.
impl ReprC for u8 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for i8 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for u16 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for i16 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for u32 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for i32 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for u64 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for u128 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for i128 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for i64 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for char {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for f32 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for f64 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}
impl ReprC for usize {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::no()
    } // Doesn't have a fixed size
}
impl ReprC for isize {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::no()
    } // Doesn't have a fixed size
}
impl ReprC for () {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsReprC {
        IsReprC::yes()
    }
}


impl<T: WithSchema, const N: usize> WithSchema for [T; N] {
    fn schema(version: u32) -> Schema {
        Schema::Array(SchemaArray {
            item_type: Box::new(T::schema(version)),
            count: N,
        })
    }
}

impl<T: Introspect, const N: usize> Introspect for [T; N] {
    fn introspect_value(&self) -> String {
        format!("[{}; {}]", std::any::type_name::<T>(), N)
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index >= self.len() {
            None
        } else {
            Some(introspect_item(index.to_string(), &self[index]))
        }
    }
}


impl<T: ReprC, const N: usize> ReprC for [T; N] {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        T::repr_c_optimization_safe(version)
    }
}
impl<T: Serialize + ReprC, const N: usize> Serialize for [T; N] {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.version).is_false() {
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

impl<T: Deserialize + ReprC, const N: usize> Deserialize for [T; N] {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        if unsafe{T::repr_c_optimization_safe(deserializer.file_version)}.is_false() {
            let mut data: [MaybeUninit<T>; N] = unsafe {
                MaybeUninit::uninit().assume_init() //This seems strange, but is correct according to rust docs: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html, see chapter 'Initializing an array element-by-element'
            };
            for idx in 0..N {
                data[idx] = MaybeUninit::new(T::deserialize(deserializer)?); //This leaks on panic, but we shouldn't panic and at least it isn't UB!
            }
            let ptr = &mut data as *mut _ as *mut [T; N];
            let res = unsafe { ptr.read() };
            core::mem::forget(data);
            Ok(res)
        } else {
            let mut data: [MaybeUninit<T>; N] = unsafe {
                MaybeUninit::uninit().assume_init() //This seems strange, but is correct according to rust docs: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html, see chapter 'Initializing an array element-by-element'
            };

            {
                let ptr = data.as_mut_ptr();
                let num_bytes: usize = std::mem::size_of::<T>() * N;
                let slice: &mut [MaybeUninit<u8>] =
                    unsafe { std::slice::from_raw_parts_mut(ptr as *mut MaybeUninit<u8>, num_bytes) };
                deserializer.reader.read_exact(unsafe { std::mem::transmute(slice) })?;
            }
            let ptr = &mut data as *mut _ as *mut [T; N];
            let res = unsafe { ptr.read() };
            core::mem::forget(data);
            Ok(res)
        }
    }
}

impl<T1> ReprC for Range<T1> {}
impl<T1: WithSchema> WithSchema for Range<T1> {
    fn schema(version: u32) -> Schema {
        Schema::new_tuple2::<T1, T1>(version)
    }
}
impl<T1: Serialize> Serialize for Range<T1> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.start.serialize(serializer)?;
        self.end.serialize(serializer)?;
        Ok(())
    }
}
impl<T1: Deserialize> Deserialize for Range<T1> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(T1::deserialize(deserializer)?..T1::deserialize(deserializer)?)
    }
}
impl<T1: Introspect> Introspect for Range<T1> {
    fn introspect_value(&self) -> String {
        return "Range".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            return Some(introspect_item("start".to_string(), &self.start));
        }
        if index == 1 {
            return Some(introspect_item("end".to_string(), &self.end));
        }
        return None;
    }
}

impl<T1:ReprC> ReprC for (T1,) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        if offset_of_tuple!((T1,),0) == 0 && std::mem::size_of::<T1>() == std::mem::size_of::<(T1, )>() {
            T1::repr_c_optimization_safe(version)
        } else {
            IsReprC::no()
        }
    }
}
impl<T1:ReprC, T2:ReprC> ReprC for (T1, T2) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        if offset_of_tuple!((T1,T2),0) == 0 && std::mem::size_of::<T1>()+std::mem::size_of::<T2>() == std::mem::size_of::<(T1, T2)>() {
            T1::repr_c_optimization_safe(version) & T2::repr_c_optimization_safe(version)
        } else {
            IsReprC::no()
        }
    }
}
impl<T1:ReprC, T2:ReprC, T3:ReprC> ReprC for (T1, T2, T3) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        if offset_of_tuple!((T1,T2,T3),0) == 0 &&
            offset_of_tuple!((T1,T2,T3),1) == std::mem::size_of::<T1>() &&
            std::mem::size_of::<T1>()+std::mem::size_of::<T2>()+std::mem::size_of::<T3>() == std::mem::size_of::<(T1, T2, T3)>() {
            T1::repr_c_optimization_safe(version) & T2::repr_c_optimization_safe(version) & T3::repr_c_optimization_safe(version)
        } else {
            IsReprC::no()
        }
    }
}
impl<T1:ReprC, T2:ReprC, T3:ReprC, T4:ReprC> ReprC for (T1, T2, T3, T4) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        if offset_of_tuple!((T1,T2,T3,T4),0) == 0 &&
            offset_of_tuple!((T1,T2,T3,T4),1) == std::mem::size_of::<T1>() &&
            offset_of_tuple!((T1,T2,T3,T4),2) == std::mem::size_of::<T1>() + std::mem::size_of::<T2>() &&
            std::mem::size_of::<T1>()+std::mem::size_of::<T2>()+std::mem::size_of::<T3>()+std::mem::size_of::<T4>() == std::mem::size_of::<(T1, T2, T3, T4)>() {
            T1::repr_c_optimization_safe(version) & T2::repr_c_optimization_safe(version) & T3::repr_c_optimization_safe(version) & T4::repr_c_optimization_safe(version)
        } else {
            IsReprC::no()
        }
    }
}

impl<T1: WithSchema, T2: WithSchema, T3: WithSchema> WithSchema for (T1, T2, T3) {
    fn schema(version: u32) -> Schema {
        Schema::new_tuple3::<T1, T2, T3>(version)
    }
}
impl<T1: Serialize, T2: Serialize, T3: Serialize> Serialize for (T1, T2, T3) {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.0.serialize(serializer)?;
        self.1.serialize(serializer)?;
        self.2.serialize(serializer)
    }
}
impl<T1: Deserialize, T2: Deserialize, T3: Deserialize> Deserialize for (T1, T2, T3) {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok((
            T1::deserialize(deserializer)?,
            T2::deserialize(deserializer)?,
            T3::deserialize(deserializer)?,
        ))
    }
}

impl<T1: WithSchema, T2: WithSchema> WithSchema for (T1, T2) {
    fn schema(version: u32) -> Schema {
        Schema::new_tuple2::<T1, T2>(version)
    }
}
impl<T1: Serialize, T2: Serialize> Serialize for (T1, T2) {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.0.serialize(serializer)?;
        self.1.serialize(serializer)
    }
}
impl<T1: Deserialize, T2: Deserialize> Deserialize for (T1, T2) {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok((T1::deserialize(deserializer)?, T2::deserialize(deserializer)?))
    }
}

impl<T1: WithSchema> WithSchema for (T1,) {
    fn schema(version: u32) -> Schema {
        Schema::new_tuple1::<T1>(version)
    }
}
impl<T1: Serialize> Serialize for (T1,) {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.0.serialize(serializer)
    }
}
impl<T1: Deserialize> Deserialize for (T1,) {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok((T1::deserialize(deserializer)?,))
    }
}


#[cfg(feature="arrayvec")]
impl<const C:usize> ReprC for arrayvec::ArrayString<C> {}


#[cfg(feature="arrayvec")]
impl<const C:usize> WithSchema for arrayvec::ArrayString<C> {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string)
    }
}
#[cfg(feature="arrayvec")]
impl<const C:usize> Serialize for arrayvec::ArrayString<C> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(self.as_str())
    }
}
#[cfg(feature="arrayvec")]
impl<const C:usize>  Deserialize for arrayvec::ArrayString<C> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize()?;
        if l > C {
            return Err(SavefileError::ArrayvecCapacityError {msg: format!("Deserialized data had length {}, but ArrayString capacity is {}", l,C)});
        }
        let mut tempbuf = [0u8;C];
        deserializer.read_bytes_to_buf(&mut tempbuf[0..l])?;

        match std::str::from_utf8(&tempbuf[0..l]) {
            Ok(s) => Ok(arrayvec::ArrayString::try_from(s)?),
            Err(_err) => Err(SavefileError::InvalidUtf8 {msg:format!("ArrayString<{}> contained invalid UTF8", C)})
        }
    }
}
#[cfg(feature="arrayvec")]
impl<const C: usize> Introspect for arrayvec::ArrayString<C> {
    fn introspect_value(&self) -> String {
        self.to_string()
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a>>> {
        None
    }
}

#[cfg(feature="arrayvec")]
impl<V: WithSchema, const C: usize> WithSchema for arrayvec::ArrayVec<V,C> {
    fn schema(version: u32) -> Schema {
        Schema::Vector(Box::new(V::schema(version)))
    }
}

#[cfg(feature="arrayvec")]
impl<V: Introspect + 'static, const C: usize> Introspect for arrayvec::ArrayVec<V,C> {
    fn introspect_value(&self) -> String {
        return "arrayvec[]".to_string();
    }

    fn introspect_child<'s>(&'s self, index: usize) -> Option<Box<dyn IntrospectItem<'s> + 's>> {
        if index >= self.len() {
            return None;
        }
        return Some(Box::new(IntrospectItemSimple {
            key: index.to_string(),
            val: &self[index],
        }));
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

#[cfg(feature="arrayvec")]
impl<V:ReprC, const C: usize> ReprC for arrayvec::ArrayVec<V,C> {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        V::repr_c_optimization_safe(version)
    }
}


#[cfg(feature="arrayvec")]
impl<V: Serialize + ReprC, const C:usize> Serialize for arrayvec::ArrayVec<V,C> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if V::repr_c_optimization_safe(serializer.version).is_false() {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<V>() * l,
                ))
            }
        }
    }
}

#[cfg(feature="arrayvec")]
impl<V: Deserialize + ReprC, const C: usize > Deserialize for arrayvec::ArrayVec<V,C> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<arrayvec::ArrayVec<V,C>, SavefileError> {
        let mut ret = arrayvec::ArrayVec::new();
        let l = deserializer.read_usize()?;
        if l > ret.capacity() {
            return Err(SavefileError::ArrayvecCapacityError {
                msg: format!("ArrayVec with capacity {} can't hold {} items", ret.capacity(), l),
            });
        }
        if unsafe{V::repr_c_optimization_safe(deserializer.file_version)}.is_false() {
            for _ in 0..l {
                ret.push(V::deserialize(deserializer)?);
            }
        } else {
            unsafe {
                let bytebuf = std::slice::from_raw_parts_mut(ret.as_mut_ptr() as *mut u8, std::mem::size_of::<V>() * l);
                deserializer.reader.read_exact(bytebuf)?; //We 'leak' ReprC objects here on error, but the idea is they are drop-less anyway, so this has no effect
                ret.set_len(l);
            }
        }
        Ok(ret)
    }
}

use std::ops::{Deref, Range};
impl<T: WithSchema> WithSchema for Box<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<T> ReprC for Box<T> {}
impl<T: Serialize> Serialize for Box<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.deref().serialize(serializer)
    }
}
impl<T: Deserialize> Deserialize for Box<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Box::new(T::deserialize(deserializer)?))
    }
}

use std::rc::Rc;

impl<T> ReprC for Rc<T> {}
impl<T: WithSchema> WithSchema for Rc<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<T: Serialize> Serialize for Rc<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.deref().serialize(serializer)
    }
}
impl<T: Deserialize> Deserialize for Rc<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Rc::new(T::deserialize(deserializer)?))
    }
}

impl<T> ReprC for Arc<T> {}
impl<T: WithSchema> WithSchema for Arc<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<T: Serialize> Serialize for Arc<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.deref().serialize(serializer)
    }
}
impl<T: Deserialize> Deserialize for Arc<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Arc::new(T::deserialize(deserializer)?))
    }
}
#[cfg(feature="bzip2")]
use bzip2::Compression;
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::cell::RefCell;
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;


use byteorder::{ReadBytesExt, WriteBytesExt};
use memoffset::offset_of_tuple;

impl<T> ReprC for RefCell<T> {}
impl<T: WithSchema> WithSchema for RefCell<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<T: Serialize> Serialize for RefCell<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.borrow().serialize(serializer)
    }
}
impl<T: Deserialize> Deserialize for RefCell<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(RefCell::new(T::deserialize(deserializer)?))
    }
}

impl<T: ReprC> ReprC for Cell<T> {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsReprC {
        T::repr_c_optimization_safe(version)
    }
}
impl<T: WithSchema> WithSchema for Cell<T> {
    fn schema(version: u32) -> Schema {
        T::schema(version)
    }
}
impl<T: Serialize + Copy> Serialize for Cell<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let t: T = self.get();
        t.serialize(serializer)
    }
}
impl<T: Deserialize> Deserialize for Cell<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Cell::new(T::deserialize(deserializer)?))
    }
}

impl WithSchema for () {
    fn schema(_version: u32) -> Schema {
        Schema::ZeroSize
    }
}
impl Serialize for () {
    fn serialize(&self, _serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        Ok(())
    }
}

impl Introspect for () {
    fn introspect_value(&self) -> String {
        "()".to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Deserialize for () {
    fn deserialize(_deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(())
    }
}

impl<T: Introspect> Introspect for (T,) {
    fn introspect_value(&self) -> String {
        return "1-tuple".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            return Some(introspect_item("0".to_string(), &self.0));
        }
        return None;
    }
}

impl<T1: Introspect, T2: Introspect> Introspect for (T1, T2) {
    fn introspect_value(&self) -> String {
        return "2-tuple".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            return Some(introspect_item("0".to_string(), &self.0));
        }
        if index == 1 {
            return Some(introspect_item("1".to_string(), &self.1));
        }
        return None;
    }
}
impl<T1: Introspect, T2: Introspect, T3: Introspect> Introspect for (T1, T2, T3) {
    fn introspect_value(&self) -> String {
        return "3-tuple".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            return Some(introspect_item("0".to_string(), &self.0));
        }
        if index == 1 {
            return Some(introspect_item("1".to_string(), &self.1));
        }
        if index == 2 {
            return Some(introspect_item("2".to_string(), &self.2));
        }
        return None;
    }
}
impl<T1: Introspect, T2: Introspect, T3: Introspect, T4: Introspect> Introspect for (T1, T2, T3, T4) {
    fn introspect_value(&self) -> String {
        return "4-tuple".to_string();
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if index == 0 {
            return Some(introspect_item("0".to_string(), &self.0));
        }
        if index == 1 {
            return Some(introspect_item("1".to_string(), &self.1));
        }
        if index == 2 {
            return Some(introspect_item("2".to_string(), &self.2));
        }
        if index == 3 {
            return Some(introspect_item("3".to_string(), &self.3));
        }
        return None;
    }
}

impl Introspect for AtomicBool {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicU8 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicI8 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicU16 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicI16 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicU32 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicI32 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicU64 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicI64 {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicUsize {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for AtomicIsize {
    fn introspect_value(&self) -> String {
        self.load(Ordering::SeqCst).to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

impl WithSchema for AtomicBool {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_bool)
    }
}
impl WithSchema for AtomicU8 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u8)
    }
}
impl WithSchema for AtomicI8 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i8)
    }
}
impl WithSchema for AtomicU16 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u16)
    }
}
impl WithSchema for AtomicI16 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i16)
    }
}
impl WithSchema for AtomicU32 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u32)
    }
}
impl WithSchema for AtomicI32 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i32)
    }
}
impl WithSchema for AtomicU64 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u64)
    }
}
impl WithSchema for AtomicI64 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i64)
    }
}
impl WithSchema for AtomicUsize {
    fn schema(_version: u32) -> Schema {
        match std::mem::size_of::<usize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_u32),
            8 => Schema::Primitive(SchemaPrimitive::schema_u64),
            _ => panic!("Size of usize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}
impl WithSchema for AtomicIsize {
    fn schema(_version: u32) -> Schema {
        match std::mem::size_of::<isize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_i32),
            8 => Schema::Primitive(SchemaPrimitive::schema_i64),
            _ => panic!("Size of isize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}

impl WithSchema for bool {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_bool)
    }
}
impl WithSchema for u8 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u8)
    }
}
impl WithSchema for i8 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i8)
    }
}
impl WithSchema for u16 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u16)
    }
}
impl WithSchema for i16 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i16)
    }
}
impl WithSchema for u32 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u32)
    }
}
impl WithSchema for i32 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i32)
    }
}
impl WithSchema for u64 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u64)
    }
}
impl WithSchema for u128 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u128)
    }
}
impl WithSchema for i128 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i128)
    }
}
impl WithSchema for i64 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i64)
    }
}
impl WithSchema for char {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_char)
    }
}
impl WithSchema for usize {
    fn schema(_version: u32) -> Schema {
        match std::mem::size_of::<usize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_u32),
            8 => Schema::Primitive(SchemaPrimitive::schema_u64),
            _ => panic!("Size of usize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}
impl WithSchema for isize {
    fn schema(_version: u32) -> Schema {
        match std::mem::size_of::<isize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_i32),
            8 => Schema::Primitive(SchemaPrimitive::schema_i64),
            _ => panic!("Size of isize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}
impl WithSchema for f32 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_f32)
    }
}
impl WithSchema for f64 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_f64)
    }
}

impl Introspect for bool {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for u8 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for u16 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for u32 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for u64 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for u128 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for i8 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for i16 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for i32 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for i64 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for char {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for i128 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for f32 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for f64 {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for usize {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl Introspect for isize {
    fn introspect_value(&self) -> String {
        self.to_string()
    }
    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

impl Serialize for u8 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u8(*self)
    }
}
impl Deserialize for u8 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u8()
    }
}
impl Serialize for bool {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_bool(*self)
    }
}
impl Deserialize for bool {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_bool()
    }
}

impl Serialize for f32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_f32(*self)
    }
}
impl Deserialize for f32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_f32()
    }
}

impl Serialize for f64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_f64(*self)
    }
}
impl Deserialize for f64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_f64()
    }
}

impl Serialize for i8 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i8(*self)
    }
}
impl Deserialize for i8 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i8()
    }
}
impl Serialize for u16 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u16(*self)
    }
}
impl Deserialize for u16 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u16()
    }
}
impl Serialize for i16 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i16(*self)
    }
}
impl Deserialize for i16 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i16()
    }
}

impl Serialize for u32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u32(*self)
    }
}
impl Deserialize for u32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u32()
    }
}
impl Serialize for i32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i32(*self)
    }
}
impl Deserialize for i32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i32()
    }
}


impl Serialize for u64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u64(*self)
    }
}
impl Deserialize for u64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u64()
    }
}
impl Serialize for i64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i64(*self)
    }
}
impl Serialize for char {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u32((*self).into())
    }
}
impl Deserialize for i64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i64()
    }
}
impl Deserialize for char {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let uc = deserializer.read_u32()?;
        match uc.try_into() {
            Ok(x) => Ok(x),
            Err(_) => Err(SavefileError::InvalidChar)
        }
    }
}
impl Serialize for u128 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u128(*self)
    }
}
impl Deserialize for u128 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u128()
    }
}
impl Serialize for i128 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i128(*self)
    }
}
impl Deserialize for i128 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i128()
    }
}

impl Serialize for usize {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(*self)
    }
}
impl Deserialize for usize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_usize()
    }
}
impl Serialize for isize {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_isize(*self)
    }
}
impl Deserialize for isize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_isize()
    }
}

impl Serialize for AtomicBool {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_bool(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicBool {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicBool::new(deserializer.read_bool()?))
    }
}

impl Serialize for AtomicU8 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u8(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU8 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicU8::new(deserializer.read_u8()?))
    }
}
impl Serialize for AtomicI8 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i8(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI8 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicI8::new(deserializer.read_i8()?))
    }
}
impl Serialize for AtomicU16 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u16(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU16 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicU16::new(deserializer.read_u16()?))
    }
}
impl Serialize for AtomicI16 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i16(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI16 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicI16::new(deserializer.read_i16()?))
    }
}

impl Serialize for AtomicU32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u32(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicU32::new(deserializer.read_u32()?))
    }
}
impl Serialize for AtomicI32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i32(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicI32::new(deserializer.read_i32()?))
    }
}

impl Serialize for AtomicU64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u64(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicU64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicU64::new(deserializer.read_u64()?))
    }
}
impl Serialize for AtomicI64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i64(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicI64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicI64::new(deserializer.read_i64()?))
    }
}

impl Serialize for AtomicUsize {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicUsize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicUsize::new(deserializer.read_usize()?))
    }
}
impl Serialize for AtomicIsize {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_isize(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicIsize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicIsize::new(deserializer.read_isize()?))
    }
}

impl ReprC for AtomicBool{}
impl ReprC for AtomicI8{}
impl ReprC for AtomicU8{}
impl ReprC for AtomicI16{}
impl ReprC for AtomicU16{}
impl ReprC for AtomicI32{}
impl ReprC for AtomicU32{}
impl ReprC for AtomicI64{}
impl ReprC for AtomicU64{}
impl ReprC for AtomicIsize{}
impl ReprC for AtomicUsize{}

/// Useful zero-sized marker. It serializes to a magic value,
/// and verifies this value on deserialization. Does not consume memory
/// data structure. Useful to troubleshoot broken Serialize/Deserialize implementations.
#[derive(Clone, Copy, Eq, PartialEq, Default, Debug)]
pub struct Canary1 {}
impl Canary1 {
    /// Create a new Canary1 object. Object has no contents.
    pub fn new() -> Canary1 {
        Canary1 {}
    }
}
impl Introspect for Canary1 {
    fn introspect_value(&self) -> String {
        "Canary1".to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

impl Deserialize for Canary1 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let magic = deserializer.read_u32()?;
        if magic != 0x47566843 {
            return Err(SavefileError::GeneralError {
                msg: format!(
                    "Encountered bad magic value when deserializing Canary1. Expected {} but got {}",
                    0x47566843, magic
                ),
            });
        }
        Ok(Canary1 {})
    }
}

impl Serialize for Canary1 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u32(0x47566843)
    }
}
impl ReprC for Canary1 {}
impl WithSchema for Canary1 {
    fn schema(_version: u32) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_canary1)
    }
}

#[derive(Clone, Debug)]
struct PathElement {
    key: String,
    key_disambiguator: usize,
    max_children: usize,
}

/// A helper which allows navigating an introspected object.
/// It remembers a path down into the guts of the object.
#[derive(Clone, Debug)]
pub struct Introspector {
    path: Vec<PathElement>,
    child_load_count: usize,
}

/// A command to navigate within an introspected object
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IntrospectorNavCommand {
    /// Select the given object and expand its children.
    /// Use this when you know the string name of the key you wish to expand.
    ExpandElement(IntrospectedElementKey),
    /// Select the Nth object at the given depth in the tree.
    /// Use this when you know the index of the field you wish to expand.
    SelectNth {
        /// Depth of item to select and expand
        select_depth: usize,
        /// Index of item to select and expand
        select_index: usize,
    },
    /// Don't navigate
    Nothing,
    /// Navigate one level up
    Up,
}

/// Identifies an introspected element somewhere in the introspection tree
/// of an object.
#[derive(PartialEq, Eq, Clone)]
pub struct IntrospectedElementKey {
    /// Depth in the tree. Fields on top level struct are at depth 0.
    pub depth: usize,
    /// The name of the field
    pub key: String,
    /// If several fields have the same name, the key_disambiguator is 0 for the first field,
    /// 1 for the next, etc.
    pub key_disambiguator: usize,
}
impl Default for IntrospectedElementKey {
    fn default() -> Self {
        IntrospectedElementKey {
            depth: 0,
            key: "".to_string(),
            key_disambiguator: 0,
        }
    }
}

/// A node in the introspection tree
#[derive(PartialEq, Eq, Clone)]
pub struct IntrospectedElement {
    /// Identifying key
    pub key: IntrospectedElementKey,
    /// Value of node
    pub value: String,
    /// Flag which tells if there are children below this node
    pub has_children: bool,
    /// Flag which tells if this child is selected
    pub selected: bool,
}

impl Debug for IntrospectedElementKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Key({} (at depth {}, key disambig {}))",
            self.key, self.depth, self.key_disambiguator
        )
    }
}

impl Debug for IntrospectedElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "KeyVal({} = {} (at depth {}, key disambig {}))",
            self.key.key, self.value, self.key.depth, self.key.key_disambiguator
        )
    }
}

impl Display for IntrospectedElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{} = {}", self.key.key, self.value)
    }
}

/// Ways in which introspection may fail
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IntrospectionError {
    /// The given depth value is invalid. At start of introspection,
    /// max depth value is 0, and fields of the root object are introspected. If a field
    /// is selected, a new level is expanded and max depth value is 1.
    BadDepth,
    /// The given key was not found
    UnknownKey,
    /// An attempt was made to select/expand a node which has no children.
    NoChildren,
    /// An attempt was made to select/expand a child with an index greater or equal to the number of children.
    IndexOutOfRange,
    /// An attempt was made to back up when already at the top.
    AlreadyAtTop,
}

/// All fields at a specific depth in the introspection tree
#[derive(Debug, Clone)]
pub struct IntrospectionFrame {
    /// The index of the expanded child, if any
    pub selected: Option<usize>,
    /// All fields at this level
    pub keyvals: Vec<IntrospectedElement>,
    /// True if there may have been more children, but expansion was stopped
    /// because the limit given to the Introspector was reached.
    pub limit_reached: bool,
}
/// An introspection tree. Note that each node in the tree can only have
/// one expanded field, and thus at most one child (a bit of a boring 'tree' :-) ).
#[derive(Debug, Clone)]
pub struct IntrospectionResult {
    /// The levels in the tree
    pub frames: Vec<IntrospectionFrame>,
    cached_total_len: usize,
}

impl Display for IntrospectionResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.format_result_row(f)
    }
}

impl IntrospectionResult {
    /// Indexes the result with a single index, which will reach all levels in the tree.
    /// Printing all elements in the order returned here, with indentation equal to
    /// item.key.depth, will yield a readable tree.
    pub fn total_index(&self, index: usize) -> Option<IntrospectedElement> {
        let mut cur = 0;
        self.total_index_impl(index, 0, &mut cur)
    }
    fn total_index_impl(&self, index: usize, depth: usize, cur: &mut usize) -> Option<IntrospectedElement> {
        if depth >= self.frames.len() {
            return None;
        }
        let frame = &self.frames[depth];
        {
            let mut offset = 0;
            if let Some(selection) = frame.selected {
                if index <= *cur + selection {
                    return Some(frame.keyvals[index - *cur].clone());
                }
                *cur += selection + 1;
                if let Some(result) = self.total_index_impl(index, depth + 1, cur) {
                    return Some(result);
                }
                offset = selection + 1;
            }
            if (index - *cur) + offset < frame.keyvals.len() {
                return Some(frame.keyvals[(index - *cur) + offset].clone());
            }
            *cur += frame.keyvals.len() - offset;
        }
        return None;
    }

    /// The total number of nodes in the tree.
    /// The value returns is the exclusive upper bound of valid
    /// indexes to the 'total_index'-method.
    pub fn total_len(&self) -> usize {
        self.cached_total_len
    }

    fn format_result_row(self: &IntrospectionResult, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.frames.len() == 0 {
            writeln!(f, "Introspectionresult:\n*empty*")?;
            return Ok(());
        }
        let mut idx = 0;
        let mut depth = Vec::new();

        writeln!(f, "Introspectionresult:")?;

        'outer: loop {
            let cur_row = &self.frames[depth.len()];
            if idx >= cur_row.keyvals.len() {
                if let Some(new_idx) = depth.pop() {
                    idx = new_idx;
                    continue;
                } else {
                    break;
                }
            }
            while idx < cur_row.keyvals.len() {
                let item = &cur_row.keyvals[idx];
                let is_selected = Some(idx) == cur_row.selected;
                let pad = if is_selected {
                    "*"
                } else {
                    if item.has_children {
                        ">"
                    } else {
                        " "
                    }
                };
                writeln!(f, "{:>indent$}{}", pad, item, indent = 1 + 2 * depth.len())?;
                idx += 1;
                if is_selected && depth.len() + 1 < self.frames.len() {
                    depth.push(idx);
                    idx = 0;
                    continue 'outer;
                }
            }
        }
        Ok(())
    }
}
impl Display for IntrospectedElementKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.key)
    }
}

struct OuterIntrospectItem<'a> {
    key: String,
    val: &'a dyn Introspect,
}

impl<'a> IntrospectItem<'a> for OuterIntrospectItem<'a> {
    fn key(&self) -> &str {
        &self.key
    }

    fn val(&self) -> &dyn Introspect {
        self.val
    }
}

impl Introspector {
    /// Returns a new Introspector with no limit to the number of fields introspected per level
    pub fn new() -> Introspector {
        Introspector {
            path: vec![],
            child_load_count: std::usize::MAX,
        }
    }
    /// Returns a new Introspector which will not enumerate more than 'child_load_count'
    /// elements on each level (useful for performance reasons to stop a 1 megabyte byte array
    /// from overwhelming the user of the introspector).
    pub fn new_with(child_load_count: usize) -> Introspector {
        Introspector {
            path: vec![],
            child_load_count,
        }
    }

    /// The current number of nodes in the tree.
    pub fn num_frames(&self) -> usize {
        self.path.len()
    }

    fn dive<'a>(
        &mut self,
        depth: usize,
        object: &'a dyn Introspect,
        navigation_command: IntrospectorNavCommand,
    ) -> Result<Vec<IntrospectionFrame>, IntrospectionError> {
        let mut result_vec = Vec::new();
        let mut navigation_command = Some(navigation_command);
        let mut cur_path = self.path.get(depth).cloned();
        let mut index = 0;
        let mut row = IntrospectionFrame {
            selected: None,
            keyvals: vec![],
            limit_reached: false,
        };
        let mut key_disambig_map = HashMap::new();

        let mut do_select_nth = None;

        let mut err_if_key_not_found = false;
        if let Some(navigation_command) = navigation_command.as_ref() {
            match navigation_command {
                IntrospectorNavCommand::ExpandElement(elem) => {
                    if elem.depth > self.path.len() {
                        return Err(IntrospectionError::BadDepth);
                    }
                    if depth == elem.depth {
                        self.path.drain(depth..);
                        self.path.push(PathElement {
                            key: elem.key.clone(),
                            key_disambiguator: elem.key_disambiguator,
                            max_children: self.child_load_count,
                        });
                        cur_path = self.path.get(depth).cloned();
                        err_if_key_not_found = true;
                    }
                }
                IntrospectorNavCommand::SelectNth {
                    select_depth,
                    select_index,
                } => {
                    if depth == *select_depth {
                        do_select_nth = Some(*select_index);
                    }
                }
                IntrospectorNavCommand::Nothing => {}
                IntrospectorNavCommand::Up => {}
            }
        }

        loop {
            if let Some(child_item) = object.introspect_child(index) {
                let key: String = child_item.key().into();

                let disambig_counter: &mut usize = key_disambig_map.entry(key.clone()).or_insert(0usize);
                let has_children = child_item.val().introspect_child(0).is_some();
                row.keyvals.push(IntrospectedElement {
                    key: IntrospectedElementKey {
                        depth,
                        key: key.clone(),
                        key_disambiguator: *disambig_counter,
                    },
                    value: child_item.val().introspect_value(),
                    has_children,
                    selected: false,
                });

                if Some(index) == do_select_nth {
                    self.path.push(PathElement {
                        key: key.clone(),
                        key_disambiguator: *disambig_counter,
                        max_children: self.child_load_count,
                    });
                    do_select_nth = None;
                    cur_path = self.path.last().cloned();
                }

                if let Some(cur_path_obj) = &cur_path {
                    if row.selected.is_none()
                        && cur_path_obj.key == key
                        && cur_path_obj.key_disambiguator == *disambig_counter
                    {
                        row.selected = Some(index);
                        row.keyvals.last_mut().unwrap().selected = true;
                        if has_children {
                            let mut subresult =
                                self.dive(depth + 1, child_item.val(), navigation_command.take().unwrap())?;
                            debug_assert_eq!(result_vec.len(), 0);
                            std::mem::swap(&mut result_vec, &mut subresult);
                        }
                    }
                }

                *disambig_counter += 1;
            } else {
                break;
            }

            index += 1;
            if index
                >= cur_path
                    .as_ref()
                    .map(|x| x.max_children)
                    .unwrap_or(self.child_load_count)
            {
                row.limit_reached = true;
                break;
            }
        }
        if do_select_nth.is_some() {
            if index == 0 {
                return Err(IntrospectionError::NoChildren);
            }
            return Err(IntrospectionError::IndexOutOfRange);
        }
        if err_if_key_not_found && row.selected.is_none() {
            self.path.pop().unwrap();
            return Err(IntrospectionError::UnknownKey);
        }
        result_vec.insert(0, row);
        Ok(result_vec)
    }

    /// Navigate the introspection tree using the given navigation_command, and also
    /// return the tree as an IntrospectionResult.
    pub fn do_introspect<'a>(
        &mut self,
        object: &'a dyn Introspect,
        navigation_command: IntrospectorNavCommand,
    ) -> Result<IntrospectionResult, IntrospectionError> {
        match &navigation_command {
            IntrospectorNavCommand::ExpandElement(_) => {}
            IntrospectorNavCommand::SelectNth { .. } => {}
            IntrospectorNavCommand::Nothing => {}
            IntrospectorNavCommand::Up => {
                if self.path.len() == 0 {
                    return Err(IntrospectionError::AlreadyAtTop);
                }
                self.path.pop();
            }
        }
        let frames = self.dive(0, object, navigation_command)?;

        let mut total = 0;
        for frame in &frames {
            total += frame.keyvals.len();
        }
        let accum = IntrospectionResult {
            frames: frames,
            cached_total_len: total,
        };
        Ok(accum)
    }
}
