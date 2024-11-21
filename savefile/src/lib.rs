#![allow(incomplete_features)]
#![recursion_limit = "256"]
#![cfg_attr(feature = "nightly", feature(specialization))]
#![deny(missing_docs)]
#![deny(warnings)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::box_default)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::needless_return)]
#![allow(clippy::manual_try_fold)] //Clean this up some day
#![allow(clippy::needless_range_loop)]
#![allow(clippy::len_zero)]
#![allow(clippy::new_without_default)]
#![allow(clippy::transmute_num_to_bytes)] //Clean this up some day
#![allow(clippy::manual_memcpy)] //Clean up some day
#![allow(clippy::needless_late_init)]

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
use savefile::prelude::*;
use savefile_derive::Savefile;

# #[cfg(miri)] fn main() {}
# #[cfg(not(miri))]
# fn main() {


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

# }
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
use savefile::prelude::*;
use std::path::Path;
use savefile_derive::Savefile;
# #[cfg(miri)] fn main() {}
# #[cfg(not(miri))]
# fn main() {

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
# }
```


# Behind the scenes

For Savefile to be able to load and save a type T, that type must implement traits
[crate::WithSchema], [crate::Packed], [crate::Serialize] and [crate::Deserialize] .
The custom derive macro Savefile derives all of these.

You can also implement these traits manually. Manual implementation can be good for:

1: Complex types for which the Savefile custom derive function does not work. For
example, trait objects or objects containing pointers.

2: Objects for which not all fields should be serialized, or which need complex
initialization (like running arbitrary code during deserialization).

Note that the four trait implementations for a particular type must be in sync.
That is, the Serialize and Deserialize traits must follow the schema defined
by the WithSchema trait for the type, and if the Packed trait promises a packed
layout, then the format produced by Serialize and Deserialze *must* exactly match
the in-memory format.

## WithSchema

The [crate::WithSchema] trait represents a type which knows which data layout it will have
when saved. Savefile includes the schema in the serialized data by default, but this can be disabled
by using the `save_noschema` function. When reading a file with unknown schema, it
is up to the user to guarantee that the file is actually of the correct format.

## Serialize

The [crate::Serialize] trait represents a type which knows how to write instances of itself to
a `Serializer`.

## Deserialize

The [crate::Deserialize] trait represents a type which knows how to read instances of itself from a `Deserializer`.

## Packed

The [crate::Packed] trait has an optional method that can be used to promise that the
in-memory format is identical to the savefile disk representation. If this is true,
instances of the type can be serialized by simply writing all the bytes in one go,
rather than having to visit individual fields. This can speed up saves significantly.


# Rules for managing versions

The basic rule is that the Deserialize trait implementation must be able to deserialize data from any previous version.

The WithSchema trait implementation must be able to return the schema for any previous verison.

The Serialize trait implementation only needs to support the latest version, for savefile itself
to work. However, for SavefileAbi to work, Serialize should support writing old versions.
The savefile-derive macro does support serializing old versions, with some limitations.


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

More about the savefile_default_val, default_fn and savefile_versions_as attributes below.

## The savefile_versions attribute

Rules for using the #\[savefile_versions] attribute:

 * You must keep track of what the current version of your data is. Let's call this version N.
 * You may only save data using version N (supply this number when calling `save`)
 * When data is loaded, you must supply version N as the memory-version number to `load`. Load will
    adapt the deserialization operation to the version of the serialized data.
 * The version number N is "global" (called GLOBAL_VERSION in the previous source example). All components of the saved data must have the same version.
 * Whenever changes to the data are to be made, the global version number N must be increased.
 * You may add a new field to your structs, iff you also give it a #\[savefile_versions = "N.."] attribute. N must be the new version of your data.
 * You may remove a field from your structs.
    - If previously it had no #\[savefile_versions] attribute, you must add a #\[savefile_versions = "..N-1"] attribute.
    - If it already had an attribute #[savefile_versions = "M.."], you must close its version interval using the current version of your data: #\[savefile_versions = "M..N-1"].
    - Whenever a field is removed, its type must simply be changed to `Removed<T>` where T is its previous type.
    - You may never completely remove items from your structs. Doing so removes backward-compatibility with that version. This will be detected at load.
    - For example, if you remove a field in version 3, you should add a #\[savefile_versions="..2"] attribute.
 * You may not change the type of a field in your structs, except when using the savefile_versions_as-macro.
 * You may add enum variants in future versions, but you may not change the size of the discriminant.


## The savefile_default_val attribute

The default_val attribute is used to provide a custom default value for
primitive types, when fields are added.

Example:

```
# use savefile_derive::Savefile;

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

## The savefile_default_fn attribute

The default_fn attribute allows constructing more complex values as defaults.

```
# use savefile_derive::Savefile;

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
# use savefile_derive::Savefile;

#[derive(Savefile)]
struct IgnoreExample {
 a: f64,
 b: f64,
 #[savefile_ignore]
 cached_product: f64
}
# fn main() {}

```

savefile_ignore does not stop the generator from generating an implementation for [Introspect] for the given field. To stop
this as well, also supply the attribute savefile_introspect_ignore .

## The savefile_versions_as attribute

The savefile_versions_as attribute can be used to support changing the type of a field.

Let's say the first version of our protocol uses the following struct:

```
# use savefile_derive::Savefile;

#[derive(Savefile)]
struct Employee {
 name : String,
 phone_number : u64
}
# fn main() {}

```

After a while, we realize that u64 is a bad choice for datatype for a phone number,
since it can't represent a number with leading 0, and also can't represent special characters
which sometimes appear in phone numbers, like '+' or '-' etc.

So, we change the type of phone_number to String:

```
# use savefile_derive::Savefile;

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
# use savefile_derive::Savefile;

#[derive(Savefile)]
struct Racecar {
 max_speed_kmh : u8,
}
# fn main() {}
```

We realize that we need to increase the range of the max_speed_kmh variable, and change it like this:

```
# use savefile_derive::Savefile;

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
Note: This entire chapter can safely be ignored. Savefile will, in most circumstances,
perform very well without any special work by the programmer.

Continuing the example from previous chapters, let's say we want to add a list of all
positions that our player have visited, so that we can provide an instant-replay function to
our game. The list can become really long, so we want to make sure that the overhead when
serializing this is as low as possible.


```
use savefile::prelude::*;
use std::path::Path;
use savefile_derive::Savefile;
# #[cfg(miri)] fn main() {}
# #[cfg(not(miri))]
# fn main() {


#[derive(Clone, Copy, Savefile)]
#[repr(C)] // Memory layout will become equal to savefile disk format - optimization possible!
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
# }
```

Savefile can speed up serialization of arrays/vectors of certain types, when it can
detect that the type consists entirely of packed plain binary data.

The above will be very fast, even if 'history' contains millions of position-instances.


Savefile has a trait [crate::Packed] that must be implemented for each T. The savefile-derive
macro implements this automatically.

This trait has an unsafe function [crate::Packed::repr_c_optimization_safe] which answers the question:
 "Is this type such that it can safely be copied byte-per-byte"?
Answering yes for a specific type T, causes savefile to optimize serialization of `Vec<T>` into being
a very fast, raw memory copy.
The exact criteria is that the in-memory representation of the type must be identical to what
the Serialize trait does for the type.


Most of the time, the user doesn't need to implement Packed, as it can be derived automatically
by the savefile derive macro.

However, implementing it manually can be done, with care. You, as implementor of the `Packed`
trait take full responsibility that all the following rules are upheld:

* The type T is Copy
* The in-memory representation of T is identical to the savefile disk format.
* The host platform is little endian. The savefile disk format uses little endian.
* The type is represented in memory in an ordered, packed representation. Savefile is not

clever enough to inspect the actual memory layout and adapt to this, so the memory representation
has to be all the types of the struct fields in a consecutive sequence without any gaps. Note
that the #\[repr(C)] attribute is not enough to do this - it will include padding if needed for alignment
reasons. You should not use #\[repr(packed)], since that may lead to unaligned struct fields.
Instead, you should use #\[repr(C)] combined with manual padding, if necessary.
If the type is an enum, it must be #\[repr(u8)], #\[repr(u16)] or #\[repr(u32)].
Enums with fields are not presently optimized.


Regarding padding, don't do:
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

When it comes to enums, there are requirements to enable the optimization:

This enum is not optimizable, since it doesn't have a defined discrminant size:
```
enum BadEnum1 {
Variant1,
Variant2,
}
```

This will be optimized:
```
#[repr(u8)]
enum GoodEnum1 {
Variant1,
Variant2,
}
```

This also:
```
#[repr(u8)]
enum GoodEnum2 {
Variant1(u8),
Variant2(u8),
}
```

However, the following will not be optimized, since there will be padding after Variant1.
To have the optimization enabled, all variants must be the same size, and without any padding.

```
#[repr(u8)]
enum BadEnum2 {
Variant1,
Variant2(u8),
}
```

This can be fixed with manual padding:
```
#[repr(u8)]
enum BadEnum2Fixed {
Variant1{padding:u8},
Variant2(u8),
}
```


This will be optimized:
```
#[repr(u8)]
enum GoodEnum3 {
Variant1{x:u8,y:u16,z:u16,w:u16},
Variant2{x:u8,y:u16,z:u32},
}
```

However, the following will not be:
```
#[repr(u8,C)]
enum BadEnum3 {
Variant1{x:u8,y:u16,z:u16,w:u16},
Variant2{x:u8,y:u16,z:u32},
}
```
The reason is that the format `#[repr(u8,C)]` will layout the struct as if the fields of each
variant were a C-struct. This means alignment of Variant2 will be 4, and the offset of 'x' will be 4.
This in turn means there will be padding between the discriminant and the fields, making the optimization
impossible.


### The attribute savefile_require_fast

When deriving the savefile-traits automatically, specify the attribute ```#[savefile_require_fast]``` to require
the optimized behaviour. If the type doesn't fulfill the required characteristics, a diagnostic will be printed in
many situations. Presently, badly aligned data structures are detected at compile time. Other problems are
only detected at runtime, and result in lower performance but still correct behaviour.
Using 'savefile_require_fast' is not unsafe, although it used to be in an old version.
Since the speedups it produces are now produced regardless, it is mostly recommended to not use
savefile_require_fast, unless compilation failure on bad alignment is desired.


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
to/from the binary stream. It is important that the Schema accurately describes the format
produced by Serialize and expected by Deserialize. Deserialization from a file is always sound,
even if the Schema is wrong. However, the process may allocate too much memory, and data
deserialized may be gibberish.

When the Schema is used by the savefile-abi crate, unsoundness can occur if the Schema is
incorrect. However, the responsibility for ensuring correctness falls on the savefile-abi crate.
The savefile-library itself produces correct Schema-instances for all types it supports.

````rust
use savefile::prelude::*;
pub struct MyPathBuf {
 path: String,
}
impl WithSchema for MyPathBuf {
 fn schema(_version: u32, context: &mut WithSchemaContext) -> Schema {
     Schema::Primitive(SchemaPrimitive::schema_string((Default::default())))
 }
}
impl Packed for MyPathBuf {
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
through the trait [Introspect]. Any type implementing this can be introspected.

The savefile-derive crate supports automatically generating an implementation for most types.

The introspection is purely 'read only'. There is no provision for using the framework to mutate
data.

Here is an example of using the trait directly:


````rust
use savefile::prelude::*;
use savefile_derive::Savefile;
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
# use savefile::prelude::*;
# use savefile_derive::Savefile;
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

use savefile_derive::Savefile;
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
manually, check that you've implemented both ```[crate::prelude::Deserialize]``` and ```[crate::prelude::Packed]```.
Without Packed, vectors cannot be deserialized, since savefile can't determine if they are safe to serialize
through simple copying of bytes.


*/

/// The prelude contains all definitions thought to be needed by typical users of the library
pub mod prelude;

#[cfg(feature = "serde_derive")]
extern crate serde;
#[cfg(feature = "serde_derive")]
extern crate serde_derive;

use core::str::Utf8Error;
#[cfg(feature = "serde_derive")]
use serde_derive::{Deserialize, Serialize};
use std::any::TypeId;

#[cfg(feature = "quickcheck")]
extern crate quickcheck;

extern crate alloc;
#[cfg(feature = "arrayvec")]
extern crate arrayvec;
extern crate byteorder;
#[cfg(feature = "parking_lot")]
extern crate parking_lot;
#[cfg(feature = "smallvec")]
extern crate smallvec;

#[cfg(feature = "parking_lot")]
use parking_lot::{Mutex, MutexGuard, RwLock, RwLockReadGuard};

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::io::{ErrorKind, Write};
use std::sync::atomic::{
    AtomicBool, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicU16, AtomicU32, AtomicU64, AtomicU8,
    AtomicUsize, Ordering,
};

pub use ::byteorder::LittleEndian;
use std::collections::BinaryHeap;
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;
#[allow(unused_imports)]
use std::mem::MaybeUninit;

#[cfg(feature = "indexmap")]
extern crate indexmap;
#[cfg(feature = "indexmap")]
use indexmap::{IndexMap, IndexSet};

#[cfg(feature = "quickcheck")]
use quickcheck::{Arbitrary, Gen};

#[cfg(feature = "bit-vec")]
extern crate bit_vec;
#[cfg(feature = "bzip2")]
extern crate bzip2;

#[cfg(feature = "bit-set")]
extern crate bit_set;

#[cfg(feature = "rustc-hash")]
extern crate rustc_hash;

extern crate memoffset;

#[cfg(feature = "derive")]
extern crate savefile_derive;

/// The current savefile version.
///
/// This versions number is used for serialized schemas.
/// There is an ambition that savefiles created by earlier versions
/// will be possible to open using later versions. The other way
/// around is not supported.
pub const CURRENT_SAVEFILE_LIB_VERSION: u16 = 2;

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
        method_name: String,
    },
    /// Savefile ABI only supports at most 63 arguments per function
    TooManyArguments,
    /// An ABI callee panicked
    CalleePanic {
        /// Descriptive message
        msg: String,
    },
    /// Loading an extern library failed (only relevant for savefile-abi)
    LoadLibraryFailed {
        /// The library being loaded
        libname: String,
        /// Possible descriptive message
        msg: String,
    },
    /// Loading an extern library failed (only relevant for savefile-abi)
    LoadSymbolFailed {
        /// The library being loaded
        libname: String,
        /// The symbol being loaded
        symbol: String,
        /// Possible descriptive message
        msg: String,
    },
}
impl From<Utf8Error> for SavefileError {
    fn from(value: Utf8Error) -> Self {
        SavefileError::InvalidUtf8 {
            msg: format!("{:?}", value),
        }
    }
}
impl Display for SavefileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SavefileError::IncompatibleSchema { message } => {
                write!(f, "Incompatible schema: {}", message)
            }
            SavefileError::IOError { io_error } => {
                write!(f, "IO error: {}", io_error)
            }
            SavefileError::InvalidUtf8 { msg } => {
                write!(f, "Invalid UTF-8: {}", msg)
            }
            SavefileError::MemoryAllocationLayoutError => {
                write!(f, "Memory allocation layout error")
            }
            SavefileError::ArrayvecCapacityError { msg } => {
                write!(f, "Arrayvec capacity error: {}", msg)
            }
            SavefileError::ShortRead => {
                write!(f, "Short read")
            }
            SavefileError::CryptographyError => {
                write!(f, "Cryptography error")
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
            SavefileError::IncompatibleSavefileLibraryVersion => {
                write!(f, "Incompatible savefile library version. Perhaps a plugin was loaded that is a future unsupported version?")
            }
            SavefileError::MissingMethod { method_name } => {
                write!(f, "Plugin is missing method {}", method_name)
            }
            SavefileError::TooManyArguments => {
                write!(f, "Function has too many arguments")
            }
            SavefileError::CalleePanic { msg } => {
                write!(f, "Invocation target panicked: {}", msg)
            }
            SavefileError::LoadLibraryFailed { libname, msg } => {
                write!(f, "Failed while loading library {}: {}", libname, msg)
            }
            SavefileError::LoadSymbolFailed { libname, symbol, msg } => {
                write!(
                    f,
                    "Failed while loading symbol {} from library {}: {}",
                    symbol, libname, msg
                )
            }
        }
    }
}

impl std::error::Error for SavefileError {}

impl SavefileError {
    /// Construct a SavefileError::GeneralError using the given string
    pub fn general(something: impl Display) -> SavefileError {
        SavefileError::GeneralError {
            msg: format!("{}", something),
        }
    }
}

/// Object to which serialized data is to be written.
///
/// This is basically just a wrapped `std::io::Write` object
/// and a file protocol version number.
/// In versions prior to 0.15, 'Serializer' did not accept a type parameter.
/// It now requires a type parameter with the type of writer to operate on.
pub struct Serializer<'a, W: Write> {
    /// The underlying writer. You should not access this.
    pub writer: &'a mut W,
    /// The version of the data structures which we are writing to disk.
    /// If this is < memory_version, we're serializing into an older format.
    /// Serializing into a future format is logically impossible.
    pub file_version: u32,
    /// State
    pub ephemeral_state: HashMap<TypeId, Box<dyn Any>>,
}

/// Object from which bytes to be deserialized are read.
///
/// This is basically just a wrapped `std::io::Read` object,
/// the version number of the file being read, and the
/// current version number of the data structures in memory.
pub struct Deserializer<'a, R: Read> {
    /// The wrapped reader
    pub reader: &'a mut R,
    /// The version of the input file
    pub file_version: u32,
    /// This contains ephemeral state that can be used to implement de-duplication of
    /// strings or possibly other situations where it is desired to deserialize DAGs.
    pub ephemeral_state: HashMap<TypeId, Box<dyn Any>>,
}

impl<TR: Read> Deserializer<'_, TR> {
    /// Get deserializer state.
    ///
    /// This function constructs a temporary state object of type R, and returns a mutable
    /// reference to it. This object can be used to store data that needs to live for the entire
    /// deserialization session. An example is de-duplicating Arc and other reference counted objects.
    /// Out of the box, `Arc<str>` has this deduplication done for it.
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
/// for the "Packed"-optimization.
#[derive(Default, Debug)]
pub struct IsPacked(bool);

#[doc(hidden)]
#[deprecated(since = "0.17.0", note = "The 'IsReprC' type has been renamed to 'IsPacked'.")]
pub type IsReprC = IsPacked;

impl std::ops::BitAnd<IsPacked> for IsPacked {
    type Output = IsPacked;

    fn bitand(self, rhs: Self) -> Self::Output {
        IsPacked(self.0 && rhs.0)
    }
}

impl IsPacked {
    /// # Safety
    /// Must only ever be created and immediately returned from
    /// Packed::repr_c_optimization_safe. Any other use, such
    /// that the value could conceivably be smuggled to
    /// a repr_c_optimization_safe-implementation is forbidden.
    ///
    /// Also, see description of Packed trait and repr_c_optimization_safe.
    pub unsafe fn yes() -> IsPacked {
        IsPacked(true)
    }
    /// No, the type is not compatible with the "Packed"-optimization.
    /// It cannot be just blitted.
    /// This is always safe, it just misses out on some optimizations.
    pub fn no() -> IsPacked {
        IsPacked(false)
    }


    /// If this returns false, "Packed"-Optimization is not allowed.
    #[inline(always)]
    pub fn is_false(self) -> bool {
        if cfg!(feature="tight") {
            true
        } else {
            !self.0
        }
    }

    /// If this returns true, "Packed"-Optimization is allowed. Beware.
    #[inline(always)]
    pub fn is_yes(self) -> bool {

        if cfg!(feature="tight") {
            false
        } else {
            self.0
        }
    }
}

/// This trait describes whether a type is such that it can just be blitted.
/// See method repr_c_optimization_safe.
/// Note! The name Packed is a little misleading. A better name would be
/// 'packed'
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be serialized or deserialized by Savefile, since it doesn't implement trait `savefile::Packed`",
        label = "This cannot be serialized or deserialized",
        note = "You can implement it by adding `#[derive(Savefile)]` before the declaration of `{Self}`",
        note = "Or you can manually implement the `savefile::Packed` trait."
    )
)]
pub trait Packed {
    /// This method returns true if the optimization is allowed
    /// for the protocol version given as an argument.
    /// This may return true if and only if the given protocol version
    /// has a serialized format identical to the memory layout of the given protocol version.
    /// Note, the only memory layout existing is that of the most recent version, so
    /// Packed-optimization only works when disk-format is identical to memory version.
    ///
    /// This can return true for types which have an in-memory layout that is packed
    /// and therefore identical to the layout that savefile will use on disk.
    /// This means that types for which this trait is implemented can be serialized
    /// very quickly by just writing their raw bits to disc.
    ///
    /// Rules to allow returning true:
    ///
    /// * The type must "be Copy" (i.e, implement the `Copy`-trait)
    /// * The type must not contain any padding (if there is padding, backward compatibility will fail, since in fallback mode regular savefile-deserialize will be used, and it will not use padding)
    /// * The type must have a strictly deterministic memory layout (no field order randomization). This typically means repr(C)
    /// * All the constituent types of the type must also implement `Packed` (correctly).
    ///
    /// Constructing an instance of 'IsPacked' with value 'true' is not safe. See
    /// documentation of 'IsPacked'. The idea is that the Packed-trait itself
    /// can still be safe to implement, it just won't be possible to get hold of an
    /// instance of IsPacked(true). That is, a safe implementation of `Packed` can't return
    /// IsPacked(true), if everything else follows safety rules. To make it impossible to just
    /// 'steal' such a value from some other thing implementing 'Packed',
    /// this method is marked unsafe (however, it can be left unimplemented,
    /// making it still possible to safely implement Packed).
    ///
    /// # Safety
    /// The returned value must not be used, except by the Savefile-framework.
    /// It must *not* be forwarded anywhere else.
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::no()
    }
}

/// This just exists to make sure that no one can actually implement the ReprC-trait placeholder.
/// If you somehow end up here, what you really want is to find instances of ReprC and change them
/// to Packed.
#[doc(hidden)]
#[deprecated(since = "0.17.0", note = "The 'ReprC' trait has been renamed to 'Packed'.")]
pub struct DeliberatelyUnimplementable {
    #[allow(dead_code)]
    private: (),
}

#[deprecated(since = "0.17.0", note = "The 'ReprC' trait has been renamed to 'Packed'.")]
#[doc(hidden)]
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "ReprC has been deprecated and must not be used. Use trait `savefile::Packed` instead!",
        label = "ReprC was erroneously required here",
        note = "Please change any `ReprC` bounds into `Packed` bounds.",
    )
)]
pub trait ReprC {
    #[deprecated(since = "0.17.0", note = "The 'ReprC' trait has been renamed to 'Packed'.")]
    #[doc(hidden)]
    #[allow(non_snake_case)]
    #[allow(deprecated)]
    fn this_is_a_placeholder__if_you_see_this_it_is_likely_that_you_have_code_that_refers_to_ReprC_trait__this_trait_has_been_renamed_to__Packed(
    ) -> DeliberatelyUnimplementable;
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::no()
    }
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
#[cfg(feature = "arrayvec")]
impl<T> From<arrayvec::CapacityError<T>> for SavefileError {
    fn from(s: arrayvec::CapacityError<T>) -> SavefileError {
        SavefileError::ArrayvecCapacityError { msg: s.to_string() }
    }
}

impl WithSchema for PathBuf {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string(VecOrStringLayout::Unknown))
    }
}
impl Serialize for PathBuf {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let as_string: String = self.to_string_lossy().to_string();
        as_string.serialize(serializer)
    }
}
impl Packed for PathBuf {}
impl Deserialize for PathBuf {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(PathBuf::from(String::deserialize(deserializer)?))
    }
}
impl Introspect for PathBuf {
    fn introspect_value(&self) -> String {
        self.to_string_lossy().to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem>> {
        None
    }
}

impl<'a, T: 'a + WithSchema + ToOwned + ?Sized> WithSchema for Cow<'a, T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        T::schema(version, context)
    }
}
impl<'a, T: 'a + ToOwned + ?Sized> Packed for Cow<'a, T> {}

impl<'a, T: 'a + Serialize + ToOwned + ?Sized> Serialize for Cow<'a, T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        (**self).serialize(serializer)
    }
}
impl<'a, T: 'a + WithSchema + ToOwned + ?Sized> Deserialize for Cow<'a, T>
where
    T::Owned: Deserialize,
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Cow<'a, T>, SavefileError> {
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

impl WithSchema for std::io::Error {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::StdIoError
    }
}
impl Packed for std::io::Error {}

impl Serialize for std::io::Error {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let kind = match self.kind() {
            ErrorKind::NotFound => 1,
            ErrorKind::PermissionDenied => 2,
            ErrorKind::ConnectionRefused => 3,
            ErrorKind::ConnectionReset => 4,
            ErrorKind::ConnectionAborted => 7,
            ErrorKind::NotConnected => 8,
            ErrorKind::AddrInUse => 9,
            ErrorKind::AddrNotAvailable => 10,
            ErrorKind::BrokenPipe => 12,
            ErrorKind::AlreadyExists => 13,
            ErrorKind::WouldBlock => 14,
            ErrorKind::InvalidInput => 21,
            ErrorKind::InvalidData => 22,
            ErrorKind::TimedOut => 23,
            ErrorKind::WriteZero => 24,
            ErrorKind::Interrupted => 36,
            ErrorKind::Unsupported => 37,
            ErrorKind::UnexpectedEof => 38,
            ErrorKind::OutOfMemory => 39,
            ErrorKind::Other => 40,
            _ => 42,
        };
        serializer.write_u16_packed(kind as u16)?;
        serializer.write_string(&self.to_string())?;
        Ok(())
    }
}
impl Deserialize for std::io::Error {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let kind = deserializer.read_u16_packed()?;
        let kind = match kind {
            1 => ErrorKind::NotFound,
            2 => ErrorKind::PermissionDenied,
            3 => ErrorKind::ConnectionRefused,
            4 => ErrorKind::ConnectionReset,
            7 => ErrorKind::ConnectionAborted,
            8 => ErrorKind::NotConnected,
            9 => ErrorKind::AddrInUse,
            10 => ErrorKind::AddrNotAvailable,
            12 => ErrorKind::BrokenPipe,
            13 => ErrorKind::AlreadyExists,
            14 => ErrorKind::WouldBlock,
            21 => ErrorKind::InvalidInput,
            22 => ErrorKind::InvalidData,
            23 => ErrorKind::TimedOut,
            24 => ErrorKind::WriteZero,
            36 => ErrorKind::Interrupted,
            37 => ErrorKind::Unsupported,
            38 => ErrorKind::UnexpectedEof,
            39 => ErrorKind::OutOfMemory,
            40 => ErrorKind::Other,
            _ => ErrorKind::Other,
        };

        let string = String::deserialize(deserializer)?;
        Ok(std::io::Error::new(kind, string))
    }
}

#[cfg(feature = "ring")]
mod crypto {
    use ring::aead;
    use ring::aead::{BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
    use ring::error::Unspecified;
    use std::fs::File;
    use std::io::{Error, ErrorKind, Read, Write};
    use std::path::Path;

    extern crate rand;

    use crate::{Deserialize, Deserializer, SavefileError, Serialize, Serializer, WithSchema};
    use byteorder::WriteBytesExt;
    use byteorder::{LittleEndian, ReadBytesExt};
    use rand::rngs::OsRng;
    use rand::RngCore;

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

    impl Drop for CryptoWriter<'_> {
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

    impl Read for CryptoReader<'_> {
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

    impl Write for CryptoWriter<'_> {
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
    pub fn save_encrypted_file<T: WithSchema + Serialize, P: AsRef<Path>>(
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
    pub fn load_encrypted_file<T: WithSchema + Deserialize, P: AsRef<Path>>(
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
#[cfg(feature = "ring")]
pub use crypto::{load_encrypted_file, save_encrypted_file, CryptoReader, CryptoWriter};



#[cfg(feature = "tight")]
impl<'a, W: Write + 'a> Serializer<'a, W> {
    /// Writes a binary little endian u16 to the output
    #[inline(always)]
    pub fn write_u16_packed(&mut self, v: u16) -> Result<(), SavefileError> {
        Ok(self.write_packed_u64_impl(v as u64)?)
    }
    /// Writes a binary little endian i16 to the output
    #[inline(always)]
    pub fn write_i16_packed(&mut self, v: i16) -> Result<(), SavefileError> {
        Ok(self.write_packed_i64_impl(v as i64)?)
    }

    /// Writes a binary little endian u32 to the output
    #[inline(always)]
    pub fn write_u32_packed(&mut self, v: u32) -> Result<(), SavefileError> {
        Ok(self.write_packed_u64_impl(v as u64)?)
    }
    /// Writes a binary little endian i32 to the output
    #[inline(always)]
    pub fn write_i32_packed(&mut self, v: i32) -> Result<(), SavefileError> {
        Ok(self.write_packed_i64_impl(v as i64)?)
    }

    /// Writes a binary little endian u64 to the output
    #[inline(always)]
    pub fn write_u64_packed(&mut self, v: u64) -> Result<(), SavefileError> {
        Ok(self.write_packed_u64_impl(v)?)
    }
    /// Writes a binary little endian i64 to the output
    #[inline(always)]
    pub fn write_i64_packed(&mut self, v: i64) -> Result<(), SavefileError> {
        Ok(self.write_packed_i64_impl(v)?)
    }
    /// Writes a binary little endian usize as u64 to the output
    #[inline(always)]
    pub fn write_usize_packed(&mut self, v: usize) -> Result<(), SavefileError> {
        Ok(self.write_packed_u64_impl(v as u64)?)
    }
    /// Writes a binary little endian isize as i64 to the output
    #[inline(always)]
    pub fn write_isize_packed(&mut self, v: isize) -> Result<(), SavefileError> {
        Ok(self.write_packed_i64_impl(v as i64)?)
    }
}
#[cfg(not(feature = "tight"))]
impl<'a, W: Write + 'a> Serializer<'a, W> {
    /// Writes a binary little endian u16 to the output
    #[inline(always)]
    pub fn write_u16_packed(&mut self, v: u16) -> Result<(), SavefileError> {
        self.write_u16(v)
    }
    /// Writes a binary little endian i16 to the output
    #[inline(always)]
    pub fn write_i16_packed(&mut self, v: i16) -> Result<(), SavefileError> {
        self.write_i16(v)
    }

    /// Writes a binary little endian u32 to the output
    #[inline(always)]
    pub fn write_u32_packed(&mut self, v: u32) -> Result<(), SavefileError> {
        self.write_u32(v)
    }
    /// Writes a binary little endian i32 to the output
    #[inline(always)]
    pub fn write_i32_packed(&mut self, v: i32) -> Result<(), SavefileError> {
        self.write_i32(v)
    }

    /// Writes a binary little endian u64 to the output
    #[inline(always)]
    pub fn write_u64_packed(&mut self, v: u64) -> Result<(), SavefileError> {
        self.write_u64(v)
    }
    /// Writes a binary little endian i64 to the output
    #[inline(always)]
    pub fn write_i64_packed(&mut self, v: i64) -> Result<(), SavefileError> {
        self.write_i64(v)
    }
    /// Writes a binary little endian usize as u64 to the output
    #[inline(always)]
    pub fn write_usize_packed(&mut self, v: usize) -> Result<(), SavefileError> {
        self.write_usize(v)
    }
    /// Writes a binary little endian isize as i64 to the output
    #[inline(always)]
    pub fn write_isize_packed(&mut self, v: isize) -> Result<(), SavefileError> {
        self.write_isize(v)
    }
}
#[cfg(not(feature = "tight"))]
const MAGIC: &'static str = "savefile\0";
#[cfg(feature = "tight")]
const MAGIC: &'static str = "";

impl<'a, W: Write + 'a> Serializer<'a, W> {
    /// Get ephemeral state of type R, for type T
    pub fn get_state<T: 'static, R: Default + 'static>(&mut self) -> &mut R {
        let type_id = TypeId::of::<T>();
        let the_any = self
            .ephemeral_state
            .entry(type_id)
            .or_insert_with(|| Box::new(R::default()));

        the_any.downcast_mut().unwrap()
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
    #[allow(unused)]
    #[inline(always)]
    fn write_packed_u64_impl(&mut self, mut val: u64) -> Result<(), SavefileError> {
        loop {
            if val < 128 {
                self.writer.write_u8(val as u8)?;
                return Ok(());
            }
            self.writer.write_u8(128|((val&127) as u8))?;
            val >>= 7;
        }
    }
    #[allow(unused)]
    #[inline(always)]
    fn write_packed_i64_impl(&mut self, val: i64) -> Result<(), SavefileError> {
        let val = val as u64;
        let val = val.rotate_left(1);
        self.write_packed_u64_impl(val)
    }

    /// Serialize the bytes of the pointer itself
    /// # Safety
    /// This method does not actually have any safety invariants.
    /// However, any realistic use case will involve a subsequent read_raw_ptr,
    /// and for that to have any chance of being sound, this call must have used
    /// a pointer to a valid T, or a null ptr.
    #[inline(always)]
    pub unsafe fn write_raw_ptr<T: ?Sized>(&mut self, data: *const T) -> Result<(), SavefileError> {
        let temp = &data as *const *const T;
        let temp_data = temp as *const u8;
        let buf = slice::from_raw_parts(temp_data, std::mem::size_of::<*const T>());
        self.write_bytes(buf)
    }

    /// Writes a binary little endian u64 to the output
    #[inline(always)]
    pub fn write_ptr(&mut self, v: *const ()) -> Result<(), SavefileError> {
        let slice_to_write = unsafe {
            std::slice::from_raw_parts(&v as *const *const () as *const u8, std::mem::size_of::<*const ()>())
        };
        Ok(self.writer.write_all(slice_to_write)?)
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
        self.write_usize_packed(asb.len())?;
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
    pub unsafe fn raw_write_region<T, T1: Packed, T2: Packed>(
        &mut self,
        full: &T,
        t1: &T1,
        t2: &T2,
        version: u32,
    ) -> Result<(), SavefileError> {
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
        Ok(Self::save_impl(
            writer,
            version,
            data,
            Some(T::schema(version, &mut WithSchemaContext::new())),
            with_compression,
            None,
        )?)
    }
    /// Creata a new serializer.
    /// Don't use this function directly, use the [crate::save_noschema] function instead.
    pub fn save_noschema<T: Serialize>(writer: &mut W, version: u32, data: &T) -> Result<(), SavefileError> {
        Ok(Self::save_impl(writer, version, data, None, false, None)?)
    }

    #[doc(hidden)]
    pub fn save_noschema_internal<T: Serialize>(
        writer: &mut W,
        version: u32,
        data: &T,
        lib_version_override: u16,
    ) -> Result<(), SavefileError> {
        Ok(Self::save_impl(
            writer,
            version,
            data,
            None,
            false,
            Some(lib_version_override),
        )?)
    }
    /// Serialize without any header. Using this means that bare_deserialize must be used to
    /// deserialize. No metadata is sent, not even version.
    pub fn bare_serialize<T: Serialize>(writer: &mut W, file_version: u32, data: &T) -> Result<(), SavefileError> {
        let mut serializer = Serializer { writer, file_version, ephemeral_state: Default::default() };
        data.serialize(&mut serializer)?;
        writer.flush()?;
        Ok(())
    }

    #[inline(always)]
    fn save_impl<T: Serialize>(
        writer: &mut W,
        version: u32,
        data: &T,
        with_schema: Option<Schema>,
        with_compression: bool,
        lib_version_override: Option<u16>,
    ) -> Result<(), SavefileError> {
        let header = MAGIC.to_string().into_bytes();

        writer.write_all(&header)?; //9

        if !cfg!(feature="tight") {
            writer.write_u16::<LittleEndian>(
                lib_version_override.unwrap_or(CURRENT_SAVEFILE_LIB_VERSION), /*savefile format version*/
            )?;
            writer.write_u32::<LittleEndian>(version)?;
        }
        // 9 + 2 + 4 = 15
        {
            if with_compression {
                if cfg!(feature="tight") {
                    panic!("compression not supported with the 'tight' feature");
                }
                writer.write_u8(1)?; //15 + 1 = 16

                #[cfg(feature = "bzip2")]
                {
                    let mut compressed_writer = bzip2::write::BzEncoder::new(writer, Compression::best());
                    if let Some(schema) = with_schema {
                        let mut schema_serializer = Serializer::<bzip2::write::BzEncoder<W>>::new_raw(
                            &mut compressed_writer,
                            lib_version_override.unwrap_or(CURRENT_SAVEFILE_LIB_VERSION) as u32,
                        );
                        schema.serialize(&mut schema_serializer)?;
                    }

                    let mut serializer = Serializer {
                        writer: &mut compressed_writer,
                        file_version: version, ephemeral_state: Default::default()

                    }; //Savefile always serializes most recent version. Only savefile-abi ever writes old formats.
                    data.serialize(&mut serializer)?;
                    compressed_writer.flush()?;
                    return Ok(());
                }
                #[cfg(not(feature = "bzip2"))]
                {
                    return Err(SavefileError::CompressionSupportNotCompiledIn);
                }
            } else {
                if !cfg!(feature="tight") {
                    writer.write_u8(0)?;
                }
                if let Some(schema) = with_schema {
                    let mut schema_serializer = Serializer::<W>::new_raw(
                        writer,
                        lib_version_override.unwrap_or(CURRENT_SAVEFILE_LIB_VERSION) as u32,
                    );
                    schema.serialize(&mut schema_serializer)?;
                }

                let mut serializer = Serializer {
                    writer,
                    file_version: version, ephemeral_state: Default::default()
                };
                data.serialize(&mut serializer)?;
                writer.flush()?;
                Ok(())
            }
        }
    }

    /// Create a Serializer.
    /// Don't use this method directly, use the [crate::save] function
    /// instead.
    pub fn new_raw(writer: &mut impl Write, file_version: u32) -> Serializer<impl Write> {
        Serializer { writer, file_version, ephemeral_state: Default::default() }
    }
}
#[cfg(not(feature="tight"))]
impl<TR: Read> Deserializer<'_, TR> {
    /// Reads a little endian u16
    pub fn read_u16_packed(&mut self) -> Result<u16, SavefileError> {
        self.read_u16()
    }
    /// Reads a little endian u32
    pub fn read_u32_packed(&mut self) -> Result<u32, SavefileError> {
        self.read_u32()
    }
    /// Reads a little endian u64
    pub fn read_u64_packed(&mut self) -> Result<u64, SavefileError> {
        self.read_u64()
    }
    /// Reads a little endian i16
    pub fn read_i16_packed(&mut self) -> Result<i16, SavefileError> {
        self.read_i16()
    }
    /// Reads a little endian i32
    pub fn read_i32_packed(&mut self) -> Result<i32, SavefileError> {
        self.read_i32()
    }
    /// Reads a little endian i64
    pub fn read_i64_packed(&mut self) -> Result<i64, SavefileError> {
        self.read_i64()
    }
    /// Reads an i64 into an isize. For 32 bit architectures, the function fails on overflow.
    pub fn read_isize_packed(&mut self) -> Result<isize, SavefileError> {
        self.read_isize()
    }
    /// Reads an u64 into an usize. For 32 bit architectures, the function fails on overflow.
    pub fn read_usize_packed(&mut self) -> Result<usize, SavefileError> {
        self.read_usize()
    }
}
#[cfg(feature="tight")]
impl<TR: Read> Deserializer<'_, TR> {
    /// Reads a little endian u16
    pub fn read_u16_packed(&mut self) -> Result<u16, SavefileError> {
        Ok(self.read_packed_u64_impl()? as u16)
    }
    /// Reads a little endian u32
    pub fn read_u32_packed(&mut self) -> Result<u32, SavefileError> {
        Ok(self.read_packed_u64_impl()? as u32)
    }
    /// Reads a little endian u64
    pub fn read_u64_packed(&mut self) -> Result<u64, SavefileError> {
        let got = self.read_packed_u64_impl()? as u64;
        Ok(got)
    }
    /// Reads a little endian i16
    pub fn read_i16_packed(&mut self) -> Result<i16, SavefileError> {
        Ok(self.read_packed_i64_impl()? as i16)
    }
    /// Reads a little endian i32
    pub fn read_i32_packed(&mut self) -> Result<i32, SavefileError> {
        Ok(self.read_packed_i64_impl()? as i32)
    }
    /// Reads a little endian i64
    pub fn read_i64_packed(&mut self) -> Result<i64, SavefileError> {
        Ok(self.read_packed_i64_impl()?)
    }
    /// Reads an i64 into an isize. For 32 bit architectures, the function fails on overflow.
    pub fn read_isize_packed(&mut self) -> Result<isize, SavefileError> {
        if let Ok(val) = TryFrom::try_from(self.read_packed_i64_impl()?) {
            Ok(val)
        } else {
            Err(SavefileError::SizeOverflow)
        }
    }
    /// Reads an u64 into an usize. For 32 bit architectures, the function fails on overflow.
    pub fn read_usize_packed(&mut self) -> Result<usize, SavefileError> {
        if let Ok(val) = TryFrom::try_from(self.read_packed_u64_impl()?) {
            Ok(val)
        } else {
            Err(SavefileError::SizeOverflow)
        }
    }
}
///
pub fn map_i64_to_u64(i: i64) -> u64 {
    if i >= 0 {
        return (i as u64)<<1;
    }
    return ((!i) as u64)<<1 | 1;
}
///
pub fn map_u64_to_i64(i: u64) -> i64 {
    if i&1 == 1 {
        let base = (i>>1) as u64;
        return (!base) as i64;
    }
    (i>>1) as i64
}

impl<TR: Read> Deserializer<'_, TR> {
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
    /// Reads an i64 into an isize. For 32 bit architectures, the function fails on overflow.
    pub fn read_isize(&mut self) -> Result<isize, SavefileError> {
        if let Ok(val) = TryFrom::try_from(self.reader.read_i64::<LittleEndian>()?) {
            Ok(val)
        } else {
            Err(SavefileError::SizeOverflow)
        }
    }
    /// Reads an u64 into an usize. For 32 bit architectures, the function fails on overflow.
    pub fn read_usize(&mut self) -> Result<usize, SavefileError> {
        if let Ok(val) = TryFrom::try_from(self.reader.read_u64::<LittleEndian>()?) {
            Ok(val)
        } else {
            Err(SavefileError::SizeOverflow)
        }
    }

    /// Reads an i8
    pub fn read_i8(&mut self) -> Result<i8, SavefileError> {
        Ok(self.reader.read_i8()?)
    }

    /// Reads an u8
    pub fn read_u8(&mut self) -> Result<u8, SavefileError> {
        Ok(self.reader.read_u8()?)
    }

    /// Reads a little endian f32
    pub fn read_f32(&mut self) -> Result<f32, SavefileError> {
        Ok(self.reader.read_f32::<LittleEndian>()?)
    }
    /// Reads a little endian f64
    pub fn read_f64(&mut self) -> Result<f64, SavefileError> {
        Ok(self.reader.read_f64::<LittleEndian>()?)
    }
    /// Reads a little endian u128
    pub fn read_u128(&mut self) -> Result<u128, SavefileError> {
        Ok(self.reader.read_u128::<LittleEndian>()?)
    }
    /// Reads a little endian i128
    pub fn read_i128(&mut self) -> Result<i128, SavefileError> {
        Ok(self.reader.read_i128::<LittleEndian>()?)
    }
    #[allow(unused)]
    #[inline(always)]
    fn read_packed_u64_impl(&mut self) -> Result<u64, SavefileError> {
        let mut val = 0u64;
        let mut shift = 0;
        loop {
            let x = self.reader.read_u8()?;
            if x < 128 {
                val |= (x as u64) << shift;
                return Ok(val);
            }
            val |= ((x&127) as u64) << shift;
            shift+= 7;
            if shift > 63 {
                return Err(SavefileError::GeneralError {
                    msg: "corrupt integer".to_string(),
                })
            }
        }
    }


    #[allow(unused)]
    #[inline(always)]
    fn read_packed_i64_impl(&mut self) -> Result<i64, SavefileError> {
        let u = self.read_packed_u64_impl()?;
        Ok(u.rotate_right(1) as i64)
    }

    /// Reads a u8 and return true if equal to 1
    pub fn read_bool(&mut self) -> Result<bool, SavefileError> {
        Ok(self.reader.read_u8()? == 1)
    }
    /// Reads the raw bit pattern of a pointer
    /// # Safety
    /// The stream must contain a valid pointer to T.
    pub unsafe fn read_raw_ptr<T: ?Sized>(&mut self) -> Result<*const T, SavefileError> {
        let mut temp = MaybeUninit::<*const T>::zeroed();

        let temp_data = &mut temp as *mut MaybeUninit<*const T> as *mut u8;
        let temp_size = std::mem::size_of::<*const T>();
        let buf = unsafe { slice::from_raw_parts_mut(temp_data, temp_size) };

        self.read_bytes_to_buf(buf)?;

        Ok(unsafe { temp.assume_init() })
    }
    /// Reads the raw bit pattern of a pointer
    /// # Safety
    /// The stream must contain a valid pointer to T.
    pub unsafe fn read_raw_ptr_mut<T: ?Sized>(&mut self) -> Result<*mut T, SavefileError> {
        let mut temp = MaybeUninit::<*mut T>::zeroed();

        let temp_data = &mut temp as *mut MaybeUninit<*mut T> as *mut u8;
        let temp_size = std::mem::size_of::<*mut T>();
        let buf = unsafe { slice::from_raw_parts_mut(temp_data, temp_size) };

        self.read_bytes_to_buf(buf)?;

        Ok(unsafe { temp.assume_init() })
    }
    /// Reads a pointer
    pub fn read_ptr(&mut self) -> Result<*const (), SavefileError> {
        let mut ptr: MaybeUninit<*const ()> = MaybeUninit::zeroed();
        let data = ptr.as_mut_ptr();
        let target = unsafe { slice::from_raw_parts_mut(data as *mut u8, std::mem::size_of::<*const ()>()) };
        self.reader.read_exact(target)?;
        Ok(unsafe { ptr.assume_init() })
    }

    /// Reads a 64 bit length followed by an utf8 encoded string. Fails if data is not valid utf8
    pub fn read_string(&mut self) -> Result<String, SavefileError> {
        let l = self.read_usize_packed()?;
        #[cfg(feature = "size_sanity_checks")]
        {
            if l > 1_000_000 {
                return Err(SavefileError::GeneralError {
                    msg: format!("String too large: {}", l),
                });
            }
        }
        let mut v = vec![0; l];
        self.reader.read_exact(&mut v)?;
        Ok(String::from_utf8(v)?)
    }

    /// Reads 'len' raw u8 bytes as a `Vec<u8>`
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, SavefileError> {
        let mut v = vec![0; len];
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
        Deserializer::<_>::load_impl::<T>(
            reader,
            version,
            Some(|version| T::schema(version, &mut WithSchemaContext::new())),
        )
    }

    /// Deserialize an object of type T from the given reader.
    /// Don't use this method directly, use the [crate::load_noschema] function
    /// instead.
    pub fn load_noschema<T: Deserialize>(reader: &mut TR, version: u32) -> Result<T, SavefileError> {
        let dummy: Option<fn(u32) -> Schema> = None;
        Deserializer::<TR>::load_impl::<T>(reader, version, dummy)
    }

    /// Deserialize data which was serialized using 'bare_serialize'
    pub fn bare_deserialize<T: Deserialize>(reader: &mut TR, file_version: u32) -> Result<T, SavefileError> {
        let mut deserializer = Deserializer {
            reader,
            file_version,
            ephemeral_state: HashMap::new(),
        };
        Ok(T::deserialize(&mut deserializer)?)
    }

    #[inline(always)]
    fn load_impl<T: Deserialize>(
        reader: &mut TR,
        version: u32,
        expected_schema: Option<impl FnOnce(u32) -> Schema>,
    ) -> Result<T, SavefileError> {
        let mut head: [u8; MAGIC.len()] = [0u8; MAGIC.len()];
        reader.read_exact(&mut head)?;



        if head[..] != (MAGIC.to_string().into_bytes())[..] {
            return Err(SavefileError::GeneralError {
                msg: "File is not in new savefile-format.".into(),
            });
        }
        let savefile_lib_version;
        let file_ver;
        let with_compression;
        if !cfg!(feature = "tight") {
            savefile_lib_version = reader.read_u16::<LittleEndian>()?;
            if savefile_lib_version > CURRENT_SAVEFILE_LIB_VERSION {
                return Err(SavefileError::GeneralError {
                    msg: "This file has been created by a future, incompatible version of the savefile crate.".into(),
                });
            }
            file_ver = reader.read_u32::<LittleEndian>()?;

            if file_ver > version {
                return Err(SavefileError::WrongVersion {
                    msg: format!(
                        "File has later version ({}) than structs in memory ({}).",
                        file_ver, version
                    ),
                });
            }
            with_compression = reader.read_u8()? != 0;
        } else {
            savefile_lib_version = CURRENT_SAVEFILE_LIB_VERSION;
            with_compression = false;
            file_ver = version;
        }

        if with_compression {
            #[cfg(feature = "bzip2")]
            {
                let mut compressed_reader = bzip2::read::BzDecoder::new(reader);
                if let Some(memory_schema) = expected_schema {
                    let mut schema_deserializer = new_schema_deserializer(&mut compressed_reader, savefile_lib_version);
                    let memory_schema = memory_schema(file_ver);
                    let file_schema = Schema::deserialize(&mut schema_deserializer)?;

                    if let Some(err) = diff_schema(&memory_schema, &file_schema, ".".to_string(), false) {
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
                    ephemeral_state: HashMap::new(),
                };
                Ok(T::deserialize(&mut deserializer)?)
            }
            #[cfg(not(feature = "bzip2"))]
            {
                return Err(SavefileError::CompressionSupportNotCompiledIn);
            }
        } else {
            if let Some(memory_schema) = expected_schema {
                let mut schema_deserializer = new_schema_deserializer(reader, savefile_lib_version);
                let memory_schema = memory_schema(file_ver);
                let file_schema = Schema::deserialize(&mut schema_deserializer)?;

                if let Some(err) = diff_schema(&memory_schema, &file_schema, ".".to_string(), false) {
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
                ephemeral_state: HashMap::new(),
            };
            Ok(T::deserialize(&mut deserializer)?)
        }
    }
}

/// Create a Deserializer.
/// Don't use this method directly, use the [crate::load] function
/// instead.
pub fn new_schema_deserializer(reader: &mut impl Read, file_schema_version: u16) -> Deserializer<impl Read> {
    Deserializer {
        reader,
        file_version: file_schema_version as u32,
        ephemeral_state: HashMap::new(),
    }
}

/// Deserialize an instance of type T from the given `reader` .
///
/// The current type of T in memory must be equal to `version`.
/// The deserializer will use the actual protocol version in the
/// file to do the deserialization.
pub fn load<T: WithSchema + Deserialize>(reader: &mut impl Read, version: u32) -> Result<T, SavefileError> {
    Deserializer::<_>::load::<T>(reader, version)
}

/// Deserialize an instance of type T from the given u8 slice .
///
/// The current type of T in memory must be equal to `version`.
/// The deserializer will use the actual protocol version in the
/// file to do the deserialization.
pub fn load_from_mem<T: WithSchema + Deserialize>(input: &[u8], version: u32) -> Result<T, SavefileError> {
    let mut input = input;
    Deserializer::load::<T>(&mut input, version)
}

/// Write the given `data` to the `writer`.
///
/// The current version of data must be `version`.
pub fn save<T: WithSchema + Serialize>(writer: &mut impl Write, version: u32, data: &T) -> Result<(), SavefileError> {
    Serializer::save::<T>(writer, version, data, false)
}

/// Write the given `data` to the `writer`. Compresses data using 'bzip2' compression format.
///
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

/// Write the given `data` to the file. Compresses data using 'bzip2' compression format.
///
/// The current version of data must be `version`.
/// The resultant data can be loaded using the regular load_file-function (it autodetects if compressions was
/// active or not).
/// Note, this function will fail if the bzip2-feature is not enabled.
pub fn save_file_compressed<T: WithSchema + Serialize, P: AsRef<Path>>(
    path: P,
    version: u32,
    data: &T,
) -> Result<(), SavefileError> {
    let mut f = BufWriter::new(File::create(path)?);
    Serializer::save::<T>(&mut f, version, data, true)
}

/// Serialize the given data and return as a `Vec<u8>`
/// The current version of data must be `version`.
pub fn save_to_mem<T: WithSchema + Serialize>(version: u32, data: &T) -> Result<Vec<u8>, SavefileError> {
    let mut retval = Vec::new();
    Serializer::save::<T>(&mut retval, version, data, false)?;
    Ok(retval)
}

/// Like [crate::load] , but used to open files saved without schema,
/// by one of the _noschema versions of the save functions.
pub fn load_noschema<T: Deserialize>(reader: &mut impl Read, version: u32) -> Result<T, SavefileError> {
    Deserializer::<_>::load_noschema::<T>(reader, version)
}

/// Write the given `data` to the `writer`, without a schema.
///
/// The current version of data must be `version`.
/// Do this write without writing any schema to disk.
/// As long as all the serializers and deserializers
/// are correctly written, the schema is not necessary.
/// Omitting the schema saves some space in the saved file,
/// but means that any mistake in implementation of the
/// Serialize or Deserialize traits will cause hard-to-troubleshoot
/// data corruption instead of a nice error message.
pub fn save_noschema<T: Serialize>(writer: &mut impl Write, version: u32, data: &T) -> Result<(), SavefileError> {
    Serializer::save_noschema::<T>(writer, version, data)
}

/// Like [crate::load] , except it deserializes from the given file in the filesystem.
/// This is a pure convenience function.
pub fn load_file<T: WithSchema + Deserialize, P: AsRef<Path>>(filepath: P, version: u32) -> Result<T, SavefileError> {
    let mut f = BufReader::new(File::open(filepath)?);
    Deserializer::load::<T>(&mut f, version)
}

/// Like [crate::save] , except it opens a file on the filesystem and writes
/// the data to it. This is a pure convenience function.
pub fn save_file<T: WithSchema + Serialize, P: AsRef<Path>>(
    filepath: P,
    version: u32,
    data: &T,
) -> Result<(), SavefileError> {
    let mut f = BufWriter::new(File::create(filepath)?);
    Serializer::save::<T>(&mut f, version, data, false)
}

/// Like [crate::load_noschema] , except it deserializes from the given file in the filesystem.
/// This is a pure convenience function.
pub fn load_file_noschema<T: Deserialize, P: AsRef<Path>>(filepath: P, version: u32) -> Result<T, SavefileError> {
    let mut f = BufReader::new(File::open(filepath)?);
    Deserializer::load_noschema::<T>(&mut f, version)
}

/// Like [crate::save_noschema] , except it opens a file on the filesystem and writes
/// the data to it.
///
/// This is a pure convenience function.
pub fn save_file_noschema<T: Serialize, P: AsRef<Path>>(
    filepath: P,
    version: u32,
    data: &T,
) -> Result<(), SavefileError> {
    let mut f = BufWriter::new(File::create(filepath)?);
    Serializer::save_noschema::<T>(&mut f, version, data)
}

/// Context object used to keep track of recursion.
///
/// Datastructures which cannot contain recursion do not need to concern themselves with
/// this. Recursive data structures in rust require the use of Box, Vec, Arc or similar.
/// The most common of these datatypes from std are supported by savefile, and will guard
/// against recursion in a well-defined way.
/// As a user of Savefile, you only need to use this if you are implementing Savefile for
/// container or smart-pointer type.
pub struct WithSchemaContext {
    seen_types: HashMap<TypeId, usize /*depth*/>,
}

impl WithSchemaContext {
    /// Create a new empty WithSchemaContext.
    /// This is useful for calling ::schema at the top-level.
    pub fn new() -> WithSchemaContext {
        let seen_types = HashMap::new();
        WithSchemaContext { seen_types }
    }
}

impl WithSchemaContext {
    /// Use this when returning the schema of a type that can be part of a recursion.
    /// For example, given a hypothetical user-implemented type MyBox, do
    ///
    /// ```rust
    /// use savefile::{Schema, WithSchema, WithSchemaContext};
    /// #[repr(transparent)]
    /// struct MyBox<T> {
    ///    content: *const T
    /// }
    /// impl<T:WithSchema + 'static> WithSchema for MyBox<T> {
    ///     fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
    ///         context.possible_recursion::<MyBox<T>>(|context| Schema::Boxed(Box::new(T::schema(version, context))))
    ///     }
    /// }
    /// ```
    ///
    /// If recursion is detected (traversing to exactly `MyBox<T>` twice, in the above example), the method
    /// 'possible_recursion' will return Schema::Recursion, stopping the Schema instance from becoming infinitely big.
    ///
    pub fn possible_recursion<T: 'static>(&mut self, cb: impl FnOnce(&mut WithSchemaContext) -> Schema) -> Schema {
        let typeid = TypeId::of::<T>();
        let prevlen = self.seen_types.len();
        match self.seen_types.entry(typeid) {
            Entry::Occupied(occ) => {
                let present_value_depth = *occ.get();
                return Schema::Recursion(prevlen - present_value_depth);
            }
            Entry::Vacant(vac) => {
                vac.insert(prevlen);
            }
        }
        let ret = (cb)(self);
        self.seen_types.remove(&typeid);
        ret
    }
}

/// This trait must be implemented by all data structures you wish to be able to save.
///
/// It must encode the schema for the datastructure when saved using the given version number.
/// When files are saved, the schema is encoded into the file.
/// when loading, the schema is inspected to make sure that the load will safely succeed.
/// This is only for increased safety, the file format does not in fact use the schema for any other
/// purpose, the design is schema-less at the core, the schema is just an added layer of safety (which
/// can be disabled).
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` does not have a defined schema for savefile, since it doesn't implement the trait `savefile::WithSchema`",
        label = "This cannot be serialized or deserialized with a schema",
        note = "You can implement it by adding `#[derive(Savefile)]` before the declaration of `{Self}`",
        note = "Or you can manually implement the `savefile::WithSchema` trait.",
        note = "You can also use one of the `*_noschema` functions to save/load without a schema."
    )
)]
pub trait WithSchema {
    /// Returns a representation of the schema used by this Serialize implementation for the given version.
    /// The WithSchemaContext can be used to guard against recursive data structures.
    /// See documentation of WithSchemaContext.
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema;
}

/// Create a new WithSchemaContext, and then call 'schema' on type T.
/// This is a useful convenience method.
pub fn get_schema<T: WithSchema + 'static>(version: u32) -> Schema {
    T::schema(version, &mut WithSchemaContext::new())
}

/// Get the schema for a type Result<OK, ERR>, where OK and ERR
/// have the schemas given by the parameters.
pub fn get_result_schema(ok: Schema, err: Schema) -> Schema {
    Schema::Enum(SchemaEnum {
        dbg_name: "Result".to_string(),
        size: None,
        alignment: None,
        variants: vec![
            Variant {
                name: "Ok".to_string(),
                discriminant: 0,
                fields: vec![Field {
                    name: "ok".to_string(),
                    value: Box::new(ok),
                    offset: None,
                }],
            },
            Variant {
                name: "Err".to_string(),
                discriminant: 0,
                fields: vec![Field {
                    name: "err".to_string(),
                    value: Box::new(err),
                    offset: None,
                }],
            },
        ],
        discriminant_size: 1,
        has_explicit_repr: false,
    })
}

/// This trait must be implemented for all data structures you wish to be
/// able to serialize.
///
/// To actually serialize data: create a [Serializer],
/// then call serialize on your data to save, giving the Serializer
/// as an argument.
///
/// The most convenient way to implement this is to use
/// `use savefile-derive::Savefile;`
///
/// and the use #\[derive(Serialize)]
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be serialized by Savefile, since it doesn't implement the trait `savefile::Serialize`",
        label = "This cannot be serialized",
        note = "You can implement it by adding `#[derive(Savefile)]` before the declaration of `{Self}`",
        note = "Or you can manually implement the `savefile::Serialize` trait."
    )
)]
pub trait Serialize: WithSchema {
    /// Serialize self into the given serializer.
    ///
    /// In versions prior to 0.15, 'Serializer' did not accept a type parameter.
    /// It now requires a type parameter with the type of writer expected.
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError>; //TODO: Do error handling
}

/// A child of an object implementing Introspect.
///
/// Is a key-value pair. The only reason this is not
/// simply (String, &dyn Introspect) is that Mutex wouldn't be introspectable in that case.
/// Mutex needs something like `(String, MutexGuard<T>)`. By having this a trait,
/// different types can have whatever reference holder needed (MutexGuard, RefMut etc).
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be an introspected value used by Savefile, since it doesn't implement the trait `savefile::IntrospectItem`",
        label = "This cannot be the type of an introspected field value",
        note = "You can possibly implement IntrospectItem manually for the type `{Self}`, or try to use `String` instead of `{Self}`."
    )
)]
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
static THE_NULL_INTROSPECTABLE: NullIntrospectable = NullIntrospectable {};

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

impl IntrospectItem<'_> for str {
    fn key(&self) -> &str {
        self
    }

    fn val(&self) -> &dyn Introspect {
        &THE_NULL_INTROSPECTABLE
    }
}

impl IntrospectItem<'_> for String {
    fn key(&self) -> &str {
        self
    }

    fn val(&self) -> &dyn Introspect {
        &THE_NULL_INTROSPECTABLE
    }
}

/// Max number of introspect children.
///
/// As a sort of guard against infinite loops, the default 'len'-implementation only
/// ever iterates this many times. This is so that broken 'introspect_child'-implementations
/// won't cause introspect_len to iterate forever.
pub const MAX_CHILDREN: usize = 10000;

/// Gives the ability to look into an object, inspecting any children (fields).
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be introspected by Savefile, since it doesn't implement trait `savefile::Introspect`",
        label = "This cannot be introspected",
        note = "If you get this message after having used the #[savefile_ignore] attribute on a field, consider adding #[savefile_introspect_ignore].",
        note = "You can implement it by adding `#[derive(Savefile)]` or `#[derive(SavefileIntrospectOnly)]` before the declaration of `{Self}`",
        note = "Or you can manually implement the `savefile::Introspect` trait."
    )
)]
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
/// `use savefile-derive::Savefile`
///
/// and the use #\[derive(Deserialize)]
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be deserialized by Savefile, since it doesn't implement the trait `savefile::Deserialize`",
        label = "This cannot be deserialized",
        note = "You can implement it by adding `#[derive(Savefile)]` before the declaration of `{Self}`",
        note = "Or you can manually implement the `savefile::Deserialize` trait."
    )
)]
pub trait Deserialize: WithSchema + Sized {
    /// Deserialize and return an instance of Self from the given deserializer.
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError>; //TODO: Do error handling
}

/// A field is serialized according to its value.
/// The name is just for diagnostics.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct Field {
    /// Field name
    pub name: String,
    /// Field type
    pub value: Box<Schema>,
    /// The field offset within the struct, if known.
    /// This is used to determine layout compatibility between different shared libraries,
    /// when using the savefile-abi crate. A value of None means offset is not known.
    /// For fields in enums, the offset is the offset from the start of memory of the enum.
    /// For a repr(C,?)-enum, this will be the offset from the start of the discriminant.
    /// For repr(rust)-enums, the discriminant may not be at the start of the memory layout.
    /// Note - if this is !=None, then it is important that the value at the given offset
    /// is actually an instance of the type given by the schema in 'value'. Otherwise,
    /// layout compatibility calculations may fail, with catastrophic consequences.
    offset: Option<usize>,
}

impl Field {
    /// Create a new instance of field, with the given name and type
    pub fn new(name: String, value: Box<Schema>) -> Field {
        Field {
            name,
            value,
            offset: None,
        }
    }
    /// Create a new instance of field, with the given name and type.
    /// The offset is the offset of the field within its struct.
    ///
    /// # Safety
    /// The offset *must* be the correct offset of the field within its struct.
    pub unsafe fn unsafe_new(name: String, value: Box<Schema>, offset: Option<usize>) -> Field {
        Field { name, value, offset }
    }
    /// Determine if the two fields are laid out identically in memory, in their parent objects.
    pub fn layout_compatible(&self, other: &Field) -> bool {
        let (Some(offset_a), Some(offset_b)) = (self.offset, other.offset) else {
            return false;
        };
        if offset_a != offset_b {
            return false;
        }
        self.value.layout_compatible(&other.value)
    }
}

/// An array is serialized by serializing its items one by one,
/// without any padding.
/// The dbg_name is just for diagnostics.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct SchemaArray {
    /// Type of array elements
    pub item_type: Box<Schema>,
    /// Length of array
    pub count: usize,
}

impl SchemaArray {
    fn layout_compatible(&self, other: &SchemaArray) -> bool {
        if self.count != other.count {
            return false;
        }
        self.item_type.layout_compatible(&other.item_type)
    }
    fn serialized_size(&self) -> Option<usize> {
        self.item_type.serialized_size().map(|x| x * self.count)
    }
}

/// Schema for a struct.
///
/// A struct is serialized by serializing its fields one by one,
/// without any padding.
/// The dbg_name is just for diagnostics.
/// The memory format is given by size, alignment and the various
/// field offsets. If any field lacks an offset, the memory format
/// is unspecified.
#[derive(Debug, PartialEq, Clone)]
#[repr(C)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct SchemaStruct {
    /// Diagnostic value
    pub dbg_name: String,
    /// If None, the memory layout of the struct is unspecified.
    /// Otherwise, the size of the struct in memory (`std::mem::size_of::<TheStruct>()`).
    size: Option<usize>,
    /// If None, the memory layout of the struct is unspecified.
    /// Otherwise, the alignment of the struct (`std::mem::align_of::<TheStruct>()`).
    alignment: Option<usize>,
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
    /// * dbg_name: The name of the struct
    /// * fields: The fields of the struct
    pub fn new(dbg_name: String, fields: Vec<Field>) -> SchemaStruct {
        SchemaStruct {
            dbg_name,
            fields,
            size: None,
            alignment: None,
        }
    }
    /// * dbg_name: The name of the struct
    /// * fields: The fields of the struct
    /// * size: If None, the memory layout of the struct is unspecified.
    ///   Otherwise, the size of the struct in memory (`std::mem::size_of::<TheStruct>()`).
    /// * alignment: If None, the memory layout of the struct is unspecified.
    ///   Otherwise, the alignment of the struct (`std::mem::align_of::<TheStruct>()`).
    pub fn new_unsafe(
        dbg_name: String,
        fields: Vec<Field>,
        size: Option<usize>,
        alignment: Option<usize>,
    ) -> SchemaStruct {
        SchemaStruct {
            dbg_name,
            fields,
            size,
            alignment,
        }
    }

    fn layout_compatible(&self, other: &SchemaStruct) -> bool {
        if self.fields.len() != other.fields.len() {
            return false;
        }
        if self.alignment.is_none() || self.size.is_none() {
            return false;
        }
        if self.alignment != other.alignment || self.size != other.size {
            return false;
        }
        for (a, b) in self.fields.iter().zip(other.fields.iter()) {
            if !a.layout_compatible(b) {
                return false;
            }
        }
        true
    }
    fn serialized_size(&self) -> Option<usize> {
        self.fields
            .iter()
            .fold(Some(0usize), |prev, x| maybe_add(prev, x.value.serialized_size()))
    }
}

/// An enum variant is serialized as its fields, one by one,
/// without any padding.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct Variant {
    /// Name of variant
    pub name: String,
    /// Discriminant in binary file-format
    pub discriminant: u8,
    /// Fields of variant
    pub fields: Vec<Field>,
}
impl Variant {
    fn layout_compatible(&self, other: &Variant) -> bool {
        if self.discriminant != other.discriminant {
            return false;
        }
        if self.fields.len() != other.fields.len() {
            return false;
        }
        for (a, b) in self.fields.iter().zip(other.fields.iter()) {
            if !a.layout_compatible(b) {
                return false;
            }
        }
        true
    }
    fn serialized_size(&self) -> Option<usize> {
        self.fields
            .iter()
            .fold(Some(0usize), |prev, x| maybe_add(prev, x.value.serialized_size()))
    }
}

/// Schema for an enum.
///
/// An enum is serialized as its u8 variant discriminant
/// followed by all the field for that variant.
/// The name of each variant, as well as its order in
/// the enum (the discriminant), is significant.
/// The memory format is given by 'has_explicit_repr',
/// 'discriminant_size', 'size', 'alignment' and the vairous variants.
///
/// Note: If 'has_explicit_repr' is false,
/// the memory format is unspecified.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct SchemaEnum {
    /// Diagnostic name
    pub dbg_name: String,
    /// Variants of enum
    pub variants: Vec<Variant>,
    /// If this is a repr(uX)-enum, then the size of the discriminant, in bytes.
    /// Valid values are 1, 2 or 4.
    /// Otherwise, this is the number of bytes needed to represent the discriminant.
    /// In either case, this is the size of the enum in the disk-format.
    pub discriminant_size: u8,
    /// True if this enum type has a repr(uX) attribute, and thus a predictable
    /// memory layout.
    has_explicit_repr: bool,
    /// The size of the enum (`std::mem::size_of::<TheEnum>()`), if known
    size: Option<usize>,
    /// The alignment of the enum (`std::mem::align_of::<TheEnum>()`)
    alignment: Option<usize>,
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
    /// Create a new SchemaEnum instance.
    /// Arguments:
    ///
    /// * dbg_name - Name of the enum type.
    /// * discriminant_size:
    ///   If this is a repr(uX)-enum, then the size of the discriminant, in bytes.
    ///   Valid values are 1, 2 or 4.
    ///   Otherwise, this is the number of bytes needed to represent the discriminant.
    ///   In either case, this is the size of the enum in the disk-format.
    /// * variants - The variants of the enum
    ///
    pub fn new(dbg_name: String, discriminant_size: u8, variants: Vec<Variant>) -> SchemaEnum {
        SchemaEnum {
            dbg_name,
            variants,
            discriminant_size,
            has_explicit_repr: false,
            size: None,
            alignment: None,
        }
    }
    /// Create a new SchemaEnum instance.
    /// Arguments:
    ///
    /// * dbg_name - Name of the enum type.
    /// * variants - The variants of the enum
    /// * discriminant_size:
    ///   If this is a repr(uX)-enum, then the size of the discriminant, in bytes.
    ///   Valid values are 1, 2 or 4.
    ///   Otherwise, this is the number of bytes needed to represent the discriminant.
    ///   In either case, this is the size of the enum in the disk-format.
    /// * has_explicit_repr: True if this enum type has a repr(uX) attribute, and thus a predictable
    ///   memory layout.
    /// * size: The size of the enum (`std::mem::size_of::<TheEnum>()`), if known
    /// * alignment: The alignment of the enum (`std::mem::align_of::<TheEnum>()`)
    ///
    /// # Safety
    /// The argument 'has_explicit_repr' must only be true if the enum in fact has a #[repr(uX)] attribute.
    /// The size and alignment must be correct for the type.
    pub fn new_unsafe(
        dbg_name: String,
        variants: Vec<Variant>,
        discriminant_size: u8,
        has_explicit_repr: bool,
        size: Option<usize>,
        alignment: Option<usize>,
    ) -> SchemaEnum {
        SchemaEnum {
            dbg_name,
            variants,
            discriminant_size,
            has_explicit_repr,
            size,
            alignment,
        }
    }
    fn layout_compatible(&self, other: &SchemaEnum) -> bool {
        if self.has_explicit_repr == false || other.has_explicit_repr == false {
            return false;
        }
        if self.alignment.is_none() || self.size.is_none() {
            return false;
        }
        if self.alignment != other.alignment || self.size != other.size {
            return false;
        }
        if self.discriminant_size != other.discriminant_size {
            return false;
        }
        if self.variants.len() != other.variants.len() {
            return false;
        }
        for (a, b) in self.variants.iter().zip(other.variants.iter()) {
            if !a.layout_compatible(b) {
                return false;
            }
        }
        true
    }
    fn serialized_size(&self) -> Option<usize> {
        let discr_size = 1usize; //Discriminant is always 1 byte
        self.variants
            .iter()
            .fold(Some(discr_size), |prev, x| maybe_max(prev, x.serialized_size()))
    }
}

/// Schema of a primitive type.
///
/// A primitive is serialized as the little endian
/// representation of its type, except for string,
/// which is serialized as an usize length followed
/// by the string in utf8.
/// These always have a specified memory format, except
/// for String, which can in theory be unspecified.
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
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
    schema_string(VecOrStringLayout),
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
    schema_char,
}
impl SchemaPrimitive {
    fn layout_compatible(&self, other: &SchemaPrimitive) -> bool {
        if let (SchemaPrimitive::schema_string(layout1), SchemaPrimitive::schema_string(layout2)) = (self, other) {
            if *layout1 == VecOrStringLayout::Unknown || *layout2 == VecOrStringLayout::Unknown {
                return false;
            }
        }
        self == other
    }
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
            SchemaPrimitive::schema_string(_) => "String",
            SchemaPrimitive::schema_f32 => "f32",
            SchemaPrimitive::schema_f64 => "f64",
            SchemaPrimitive::schema_bool => "bool",
            SchemaPrimitive::schema_canary1 => "u32",
            SchemaPrimitive::schema_u128 => "u128",
            SchemaPrimitive::schema_i128 => "i128",
            SchemaPrimitive::schema_char => "char",
        }
    }

    fn serialized_size(&self) -> Option<usize> {
        match *self {
            SchemaPrimitive::schema_i8 | SchemaPrimitive::schema_u8 => Some(1),
            SchemaPrimitive::schema_i16 | SchemaPrimitive::schema_u16 => Some(2),
            SchemaPrimitive::schema_i32 | SchemaPrimitive::schema_u32 => Some(4),
            SchemaPrimitive::schema_i64 | SchemaPrimitive::schema_u64 => Some(8),
            SchemaPrimitive::schema_string(_) => None,
            SchemaPrimitive::schema_f32 => Some(4),
            SchemaPrimitive::schema_f64 => Some(8),
            SchemaPrimitive::schema_bool => Some(1),
            SchemaPrimitive::schema_canary1 => Some(4),
            SchemaPrimitive::schema_i128 | SchemaPrimitive::schema_u128 => Some(16),
            SchemaPrimitive::schema_char => Some(4),
        }
    }
}

fn diff_primitive(a: SchemaPrimitive, b: SchemaPrimitive, path: &str) -> Option<String> {
    if a != b {
        if let (SchemaPrimitive::schema_string(_), SchemaPrimitive::schema_string(_)) = (&a, &b) {
            return None; //Strings have the same schema, even if they're not memory-layout compatible
        }
        return Some(format!(
            "At location [{}]: Application protocol has datatype {}, but disk format has {}",
            path,
            a.name(),
            b.name()
        ));
    }
    None
}

/// The actual layout in memory of a Vec-like datastructure.
/// If this is 'Unknown', the memory format is unspecified.
/// Otherwise, it is as given by the variant.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[repr(u8)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum VecOrStringLayout {
    #[default]
    /// Nothing is known. We must assume that the memory layout could be anything
    Unknown,
    /// Data pointer, plus capacity and length usize
    DataCapacityLength,
    /// Data pointer, plus length and capacity usize
    DataLengthCapacity,
    /// One of the possible vec layouts, capacity-data-length
    CapacityDataLength,
    /// One of the possible vec layouts, length-data-capacity
    LengthDataCapacity,
    /// One of the possible vec layouts, capacity-length-data
    CapacityLengthData,
    /// One of the possible vec layouts, length-capacity-data
    LengthCapacityData,
    /// Length, then data
    LengthData,
    /// Data, then length
    DataLength,
}

impl Packed for AbiMethodArgument {}

/// The definition of an argument to a method
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct AbiMethodArgument {
    /// The schema (type) of the argument. This contains
    /// primarily the on-disk serialized format, but also
    /// contains information that can allow savefile-abi to determine
    /// if memory layouts are the same.
    pub schema: Schema,
}

impl Deserialize for AbiMethodArgument {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AbiMethodArgument {
            schema: <_ as Deserialize>::deserialize(deserializer)?,
        })
    }
}

impl WithSchema for AbiMethodArgument {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}

impl Serialize for AbiMethodArgument {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.schema.serialize(serializer)?;
        Ok(())
    }
}

/// The type of the 'self'-parameter
#[non_exhaustive]
#[derive(PartialEq, Debug, Clone, Copy)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum ReceiverType {
    /// &self
    Shared, // &self
    /// &mut self
    Mut, // &mut self
    /// self: Pin<&mut Self>
    PinMut, // self: Pin<&mut Self>
}

/// Return value and argument types for a method
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct AbiMethodInfo {
    /// The return value type of the method
    pub return_value: Schema,
    /// What type is the 'self'-parameter of this method?
    pub receiver: ReceiverType,
    /// The arguments of the method.
    pub arguments: Vec<AbiMethodArgument>,
    /// True if this method was found to have been modified by async_trait,
    /// converting it to return a boxed future.
    pub async_trait_heuristic: bool,
}

impl Packed for AbiMethodInfo {}
impl WithSchema for AbiMethodInfo {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}

impl Serialize for AbiMethodInfo {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.return_value.serialize(serializer)?;
        if serializer.file_version >= 2 {
            serializer.write_u8(match self.receiver {
                ReceiverType::Shared => 100,
                ReceiverType::Mut => 101,
                ReceiverType::PinMut => 102,
            })?;
            serializer.write_bool(self.async_trait_heuristic)?;
        }
        self.arguments.serialize(serializer)?;
        Ok(())
    }
}
impl Deserialize for AbiMethodInfo {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let return_value = <_ as Deserialize>::deserialize(deserializer)?;
        let async_trait_heuristic;
        let receiver;
        if deserializer.file_version >= 2 {
            receiver = match deserializer.read_u8()? {
                100 => ReceiverType::Shared,
                101 => ReceiverType::Mut,
                102 => ReceiverType::PinMut,
                x => return Err(SavefileError::WrongVersion {
                    msg: format!("Version 0.17.x (or earlier) of the savefile-library detected. It is not compatible with the current version. Please upgrade to version >0.18. Unexpected value: {}", x),
                }),
            };
            async_trait_heuristic = deserializer.read_bool()?;
        } else {
            receiver = ReceiverType::Shared;
            async_trait_heuristic = false;
        };
        Ok(AbiMethodInfo {
            return_value,
            receiver,
            arguments: <_ as Deserialize>::deserialize(deserializer)?,
            async_trait_heuristic,
        })
    }
}

/// A method exposed through savefile-abi.
/// Contains a name, and a signature.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct AbiMethod {
    /// The name of the method
    pub name: String,
    /// The function signature
    pub info: AbiMethodInfo,
}
impl Packed for AbiMethod {}
impl WithSchema for AbiMethod {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for AbiMethod {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.name.serialize(serializer)?;
        self.info.serialize(serializer)?;
        Ok(())
    }
}
impl Deserialize for AbiMethod {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AbiMethod {
            name: <_ as Deserialize>::deserialize(deserializer)?,
            info: <_ as Deserialize>::deserialize(deserializer)?,
        })
    }
}

/// Defines a dyn trait, basically
#[derive(Default, Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct AbiTraitDefinition {
    /// The name of the trait
    pub name: String,
    /// The set of methods available on the trait
    pub methods: Vec<AbiMethod>,
    /// True if this object is 'Sync'
    pub sync: bool,
    /// True if this object is 'Send'
    pub send: bool,
}
impl Packed for AbiTraitDefinition {}
impl WithSchema for AbiTraitDefinition {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for AbiTraitDefinition {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let mut effective_name = self.name.clone();
        if self.sync {
            effective_name += "+Sync";
        }
        if self.send {
            effective_name += "+Send";
        }
        effective_name.serialize(serializer)?;
        self.methods.serialize(serializer)?;
        Ok(())
    }
}
impl Deserialize for AbiTraitDefinition {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let name: String = <_ as Deserialize>::deserialize(deserializer)?;

        let actual_name = name.split('+').next().unwrap();
        let mut sync = false;
        let mut send = false;
        for segment in name.split('+').skip(1) {
            match segment {
                "Sync" => sync = true,
                "Send" => send = true,
                _ => panic!("Unexpected trait name encountered: {}", name),
            }
        }

        let t = AbiTraitDefinition {
            name: actual_name.to_string(),
            methods: <_ as Deserialize>::deserialize(deserializer)?,
            sync,
            send,
        };
        Ok(t)
    }
}

impl AbiTraitDefinition {
    /// Verify that the 'self' trait definition is compatible with the 'old' definition.
    /// Note, this routine ignores methods which only exist in 'self'.
    /// The motivation is that older clients won't call them. Of course, a newer client
    /// might call such a method, but this will be detected at runtime, and will panic.
    /// However, it is hard to do very much better than this.
    ///
    /// This routine will flag an error if a method that used to exist, no longer does.
    /// Note that the _existence_ of methods is not itself versioned with a version number.
    ///
    /// The version number is only for the data types of the arguments.
    ///
    /// old is the callee, self is the caller
    fn verify_compatible_with_old_impl(
        &self,
        old_version: u32,
        old: &AbiTraitDefinition,
        is_return_position: bool,
    ) -> Result<(), String> {
        if is_return_position {
            if !old.sync && self.sync {
                return Err(format!("Trait {} was not Sync in version {}, but the Sync-bound has since been added. This is not a backward-compatible change.",
                                   self.name, old_version,
                ));
            }
            if !old.send && self.send {
                return Err(format!("Trait {} was not Send in version {}, but the Send-bound has since been added. This is not a backward-compatible change.",
                                   self.name, old_version,
                ));
            }
        } else {
            if old.sync && !self.sync {
                return Err(format!("Trait {} was Sync in version {}, but the Sync-bound has since been removed. This is not a backward-compatible change.",
                                   self.name, old_version,
                ));
            }
            if old.send && !self.send {
                return Err(format!("Trait {} was Send in version {}, but the Send-bound has since been removed. This is not a backward-compatible change.",
                                   self.name, old_version,
                ));
            }
        }

        for old_method in old.methods.iter() {
            let Some(new_method) = self.methods.iter().find(|x| x.name == old_method.name) else {
                return Err(format!("In trait {}, the method {} existed in version {}, but has been removed. This is not a backward-compatible change.",
                                   self.name, old_method.name, old_version,
                ));
            };
            if new_method.info.async_trait_heuristic != old_method.info.async_trait_heuristic {
                if old_method.info.async_trait_heuristic {
                    return Err(format!("In trait {}, the method {} was previously async, using #[async_trait], but it does no longer. This is not a backward-compatible change.",
                                       self.name, old_method.name
                    ));
                } else {
                    return Err(format!("In trait {}, the method {} is now async, using #[async_trait], but it previously did not. This is not a backward-compatible change.",
                                       self.name, old_method.name
                    ));
                }
            }
            if new_method.info.arguments.len() != old_method.info.arguments.len() {
                return Err(format!("In trait {}, method {}, the number of arguments has changed from {} in version {} to {}. This is not a backward-compatible change.",
                                   self.name, old_method.name, old_method.info.arguments.len(), old_version, new_method.info.arguments.len()
                ));
            }
            if let Some(diff) = diff_schema(
                &new_method.info.return_value,
                &old_method.info.return_value,
                "".into(),
                is_return_position,
            ) {
                return Err(format!("In trait {}, method {}, the return value type has changed from version {}: {}. This is not a backward-compatible change.",
                                   self.name, old_method.name, old_version, diff
                ));
            }
            for (arg_index, (new_arg, old_arg)) in new_method
                .info
                .arguments
                .iter()
                .zip(old_method.info.arguments.iter())
                .enumerate()
            {
                if let Some(diff) = diff_schema(&new_arg.schema, &old_arg.schema, "".into(), is_return_position) {
                    return Err(format!("In trait {}, method {}, argument {}, the type has changed from version {}: {}. This is not a backward-compatible change.",
                                       self.name, old_method.name, arg_index , old_version, diff
                    ));
                }
            }
        }

        Ok(())
    }

    /// Verify that 'self' represents a newer version of a trait, that is backward compatible
    /// with 'old'. 'old_version' is the version number of the old version being inspected.
    /// To guarantee compatibility, all versions must be checked
    ///
    /// old is the callee, self is the caller
    pub fn verify_backward_compatible(
        &self,
        old_version: u32,
        old: &AbiTraitDefinition,
        is_return_position: bool,
    ) -> Result<(), SavefileError> {
        self.verify_compatible_with_old_impl(old_version, old, is_return_position)
            .map_err(|x| SavefileError::IncompatibleSchema { message: x })
    }
}

/// The schema represents the save file format
/// of your data structure.
///
/// It is a tree,
/// consisting of various types of nodes in the savefile
/// format. Custom Serialize-implementations cannot add new types to
/// this tree, but must reuse these existing ones.
/// See the various enum variants for more information.
///
/// Note, the Schema actually carries two different pieces of information
///  * The disk format
///  * The memory format. The latter is only used by SavefileAbi.
///
/// Note, schema instances may choose to not specify any memory format. If so,
/// SavefileAbi will have to resort to serialization.
///
/// Exactly how the memory format is specified varies for the variants.
/// Check the variant documentation.
#[derive(Debug, PartialEq, Clone)]
#[repr(C, u32)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum Schema {
    /// Represents a struct. Custom implementations of Serialize are encouraged to use this format.
    Struct(SchemaStruct),
    /// Represents an enum.
    Enum(SchemaEnum),
    /// Represents a primitive: Any of the various integer types (u8, i8, u16, i16 etc...), or String
    Primitive(SchemaPrimitive),
    /// A Vector of arbitrary nodes, all of the given type.
    /// This has a specified memory format unless the VecOrStringLayout value is 'Unknown'.
    Vector(Box<Schema>, VecOrStringLayout /*standard savefile memory layout*/),
    /// An array of N arbitrary nodes, all of the given type
    /// This has a specified memory format unless the VecOrStringLayout value is 'Unknown'.
    Array(SchemaArray),
    /// An Option variable instance of the given type.
    /// This has a specified memory format (a pointer to instance of 'Schema')
    SchemaOption(Box<Schema>),
    /// Basically a dummy value, the Schema nodes themselves report this schema if queried.
    /// This never has a specified memory format.
    Undefined,
    /// A zero-sized type. I.e, there is no data to serialize or deserialize.
    /// This always has a specified memory format.
    ZeroSize,
    /// A user-defined, custom type. The string can be anything. The schema
    /// only matches if the string is identical. Use with caution. Consider
    /// if your type is aptly represented as a Struct or Enum instead.
    /// This never has a specified memory format.
    Custom(String),
    /// The savefile format of a `Box<T>` is identical to that of `T`.
    /// But SavefileAbi still needs to represent `Box<T>` separate from `T`, since
    /// their memory layout isn't the same.
    Boxed(Box<Schema>),
    /// Savefile does not support deserializing unsized slices.
    /// But SavefileAbi supports these as parameters.
    /// Savefile schema still needs to be able to represent them.
    Slice(Box<Schema>),
    /// Savefile does not support deserializing &str, nor the unsized str.
    /// But SavefileAbi supports &str as parameters. It does not support str.
    /// Savefile schema still needs to be able to represent Str.
    Str,
    /// Savefile does not support deserializing references.
    /// If it would, the savefile format of `&T` would be identical to that of `T`.
    /// But SavefileAbi still needs to represent &T separate from T, since
    /// their memory layout isn't the same.
    Reference(Box<Schema>),
    /// Traits cannot be serialized, but they can be exchanged using savefile-abi
    /// Their memory layout is considered to depend on all method signatures,
    /// and the layouts of all argument types and all return types.
    Trait(bool /* mut self*/, AbiTraitDefinition),
    /// This is just a trait. But it exists as a separate schema variant,
    /// since SavefileAbi automatically generates wrappers for standard Fn*-types,
    /// and these should not be mixed up with regular trait definitions, even if they
    /// would be identical
    /// Traits cannot be serialized, but they can be exchanged using savefile-abi
    /// Their memory layout is considered to depend on all method signatures,
    /// and the layouts of all argument types and all return types.
    FnClosure(bool /*mut self*/, AbiTraitDefinition),
    /// The datastructure is recursive, and the datatype now continues from
    /// the element that is 'depth' layers higher in the schema tree.
    /// Note, the 'depth' only counts possible recursion points, i.e, objects
    /// such as 'Box', 'Vec' etc. This works, since the schema will only ever match
    /// if it is identical in memory and file, and because of this, counting
    /// only the recursion points is non-ambiguous.
    Recursion(usize /*depth*/),
    /// std::io::Error
    StdIoError,
    /// Savefile-abi boxed Future
    Future(
        AbiTraitDefinition,
        /*send*/ bool,
        /*sync*/ bool,
        /*unpin*/ bool,
    ),
}
/// Introspect is not implemented for Schema, though it could be
impl Introspect for Schema {
    fn introspect_value(&self) -> String {
        "Schema".to_string()
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
}

impl Schema {
    /// Get a short description of the major type of this schema.
    /// 'struct', 'enum' etc.
    pub fn top_level_description(&self) -> String {
        match self {
            Schema::Struct(_) => "struct".into(),
            Schema::Enum(_) => "enum".into(),
            Schema::Primitive(_) => "primitive".into(),
            Schema::Vector(_, _) => "vector".into(),
            Schema::Array(_) => "array".into(),
            Schema::SchemaOption(_) => "option".into(),
            Schema::Undefined => "undefined".into(),
            Schema::ZeroSize => "zerosize".into(),
            Schema::Custom(_) => "custom".into(),
            Schema::Boxed(_) => "box".into(),
            Schema::FnClosure(_, _) => "fntrait".into(),
            Schema::Slice(_) => "slice".into(),
            Schema::Str => "str".into(),
            Schema::Reference(_) => "reference".into(),
            Schema::Trait(_, _) => "trait".into(),
            Schema::Recursion(depth) => {
                format!("<recursion {}>", depth)
            }
            Schema::StdIoError => "stdioerror".into(),
            Schema::Future(_, _, _, _) => "future".into(),
        }
    }
    /// Determine if the two fields are laid out identically in memory, in their parent objects.
    pub fn layout_compatible(&self, b_native: &Schema) -> bool {
        match (self, b_native) {
            (Schema::Struct(a), Schema::Struct(b)) => a.layout_compatible(b),
            (Schema::Enum(a), Schema::Enum(b)) => a.layout_compatible(b),
            (Schema::Primitive(a), Schema::Primitive(b)) => a.layout_compatible(b),
            (Schema::Vector(a, a_standard_layout), Schema::Vector(b, b_standard_layout)) => {
                a.layout_compatible(b)
                    && *a_standard_layout != VecOrStringLayout::Unknown
                    && *b_standard_layout != VecOrStringLayout::Unknown
                    && *a_standard_layout == *b_standard_layout
            }
            (Schema::Array(a), Schema::Array(b)) => a.layout_compatible(b),
            (Schema::SchemaOption(_), Schema::SchemaOption(_)) => {
                false // Layout of enums in memory is undefined, and also hard to determine at runtime
            }
            (Schema::ZeroSize, Schema::ZeroSize) => true,
            (Schema::Custom(_), Schema::Custom(_)) => {
                false // Be conservative here
            }
            (Schema::FnClosure(_a1, _a2), Schema::FnClosure(_b1, _b2)) => {
                // Closures as arguments are handled differently.
                // Closures are not supported in any other position
                false
            }
            (Schema::Boxed(a), Schema::Boxed(b)) => {
                // The memory layout of boxes is guaranteed in practice (just a pointer)
                // Trait pointers (which are fat) could conceivably differ, but we don't
                // actually rely on memory layout compatibility for them, and this expression
                // will also return false (since Schema::Trait 'layout_compatible' always returns false).
                a.layout_compatible(b)
            }
            (Schema::Reference(a), Schema::Reference(b)) => a.layout_compatible(b),
            (Schema::Slice(a), Schema::Slice(b)) => a.layout_compatible(b),
            _ => false,
        }
    }
    /// Create a 1-element tuple
    pub fn new_tuple1<T1: WithSchema>(version: u32, context: &mut WithSchemaContext) -> Schema {
        let schema = Box::new(T1::schema(version, context));
        Schema::Struct(SchemaStruct {
            dbg_name: "1-Tuple".to_string(),
            size: Some(std::mem::size_of::<(T1,)>()),
            alignment: Some(std::mem::align_of::<(T1,)>()),
            fields: vec![Field {
                name: "0".to_string(),
                value: schema,
                offset: Some(offset_of_tuple!((T1,), 0)),
            }],
        })
    }

    /// Create a 2-element tuple
    pub fn new_tuple2<T1: WithSchema, T2: WithSchema>(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "2-Tuple".to_string(),
            size: Some(std::mem::size_of::<(T1, T2)>()),
            alignment: Some(std::mem::align_of::<(T1, T2)>()),
            fields: vec![
                Field {
                    name: "0".to_string(),
                    value: Box::new(T1::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2), 0)),
                },
                Field {
                    name: "1".to_string(),
                    value: Box::new(T2::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2), 1)),
                },
            ],
        })
    }
    /// Create a 3-element tuple
    pub fn new_tuple3<T1: WithSchema, T2: WithSchema, T3: WithSchema>(
        version: u32,
        context: &mut WithSchemaContext,
    ) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "3-Tuple".to_string(),
            size: Some(std::mem::size_of::<(T1, T2, T3)>()),
            alignment: Some(std::mem::align_of::<(T1, T2, T3)>()),
            fields: vec![
                Field {
                    name: "0".to_string(),
                    value: Box::new(T1::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3), 0)),
                },
                Field {
                    name: "1".to_string(),
                    value: Box::new(T2::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3), 1)),
                },
                Field {
                    name: "2".to_string(),
                    value: Box::new(T3::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3), 2)),
                },
            ],
        })
    }
    /// Create a 4-element tuple
    pub fn new_tuple4<T1: WithSchema, T2: WithSchema, T3: WithSchema, T4: WithSchema>(
        version: u32,
        context: &mut WithSchemaContext,
    ) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "4-Tuple".to_string(),
            size: Some(std::mem::size_of::<(T1, T2, T3, T4)>()),
            alignment: Some(std::mem::align_of::<(T1, T2, T3, T4)>()),
            fields: vec![
                Field {
                    name: "0".to_string(),
                    value: Box::new(T1::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3, T4), 0)),
                },
                Field {
                    name: "1".to_string(),
                    value: Box::new(T2::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3, T4), 1)),
                },
                Field {
                    name: "2".to_string(),
                    value: Box::new(T3::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3, T4), 2)),
                },
                Field {
                    name: "3".to_string(),
                    value: Box::new(T4::schema(version, context)),
                    offset: Some(offset_of_tuple!((T1, T2, T3, T4), 3)),
                },
            ],
        })
    }
    /// Size
    pub fn serialized_size(&self) -> Option<usize> {
        match self {
            Schema::Struct(ref schema_struct) => schema_struct.serialized_size(),
            Schema::Enum(ref schema_enum) => schema_enum.serialized_size(),
            Schema::Primitive(ref schema_primitive) => schema_primitive.serialized_size(),
            Schema::Vector(ref _vector, _) => None,
            Schema::Array(ref array) => array.serialized_size(),
            Schema::SchemaOption(ref _content) => None,
            Schema::Undefined => None,
            Schema::ZeroSize => Some(0),
            Schema::Custom(_) => None,
            Schema::Boxed(inner) => inner.serialized_size(),
            Schema::FnClosure(_, _) => None,
            Schema::Slice(_) => None,
            Schema::Str => None,
            Schema::Reference(_) => None,
            Schema::Trait(_, _) => None,
            Schema::Recursion(_) => None,
            Schema::StdIoError => None,
            Schema::Future(_, _, _, _) => None,
        }
    }
}

fn diff_vector(a: &Schema, b: &Schema, path: String) -> Option<String> {
    diff_schema(a, b, path + "/*", false)
}

fn diff_array(a: &SchemaArray, b: &SchemaArray, path: String) -> Option<String> {
    if a.count != b.count {
        return Some(format!(
            "At location [{}]: In memory array has length {}, but disk format length {}.",
            path, a.count, b.count
        ));
    }

    diff_schema(&a.item_type, &b.item_type, format!("{}/[{}]", path, a.count), false)
}

fn diff_option(a: &Schema, b: &Schema, path: String) -> Option<String> {
    diff_schema(a, b, path + "/?", false)
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
    if a.discriminant_size != b.discriminant_size {
        return Some(format!(
            "At location [{}]: In memory enum has a representation with {} bytes for the discriminant, but disk format has {}.",
            path,
            a.discriminant_size,
            b.discriminant_size
        ));
    }
    for i in 0..a.variants.len() {
        if a.variants[i].name != b.variants[i].name {
            return Some(format!(
                "At location [{}]: Enum variant #{} in memory is called {}, but in disk format it is called {}",
                &path, i, a.variants[i].name, b.variants[i].name
            ));
        }
        if a.variants[i].discriminant != b.variants[i].discriminant {
            return Some(format!(
                "At location [{}]: Enum variant #{} in memory has discriminant {}, but in disk format it has {}",
                &path, i, a.variants[i].discriminant, b.variants[i].discriminant
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
            false,
        );
        if let Some(err) = r {
            return Some(err);
        }
    }
    None
}
/// Return a (kind of) human-readable description of the difference
/// between the two schemas.
///
/// The schema 'a' is assumed to be the current
/// schema (used in memory).
/// Returns None if both schemas are equivalent
/// This does not care about memory layout, only serializability.
///
/// is_return_pos should be true if the schema we're diffing is for a return value in
/// savefile abi, otherwise false.
///
/// for ABI calls:
/// a is the caller
/// b is the callee
pub fn diff_schema(a: &Schema, b: &Schema, path: String, is_return_pos: bool) -> Option<String> {
    let (atype, btype) = match (a, b) {
        (Schema::Struct(a), Schema::Struct(b)) => return diff_struct(a, b, path),
        (Schema::Enum(a), Schema::Enum(b)) => return diff_enum(a, b, path),
        (Schema::Primitive(a1), Schema::Primitive(b1)) => return diff_primitive(*a1, *b1, &path),
        (Schema::Vector(a1, _a2), Schema::Vector(b1, _b2)) => return diff_vector(a1, b1, path),
        (Schema::SchemaOption(a), Schema::SchemaOption(b)) => {
            return diff_option(a, b, path);
        }
        (Schema::Undefined, Schema::Undefined) => {
            return Some(format!("At location [{}]: Undefined schema encountered.", path))
        }
        (Schema::ZeroSize, Schema::ZeroSize) => {
            return None;
        }
        (Schema::Array(a), Schema::Array(b)) => return diff_array(a, b, path),
        (Schema::Custom(a), Schema::Custom(b)) => {
            if a != b {
                return Some(format!(
                    "At location [{}]: Application protocol has datatype Custom({}), but foreign format has Custom({})",
                    path, a, b
                ));
            }
            return None;
        }
        (Schema::Str, Schema::Str) => {
            return None;
        }
        (Schema::StdIoError, Schema::StdIoError) => {
            return None;
        }
        (Schema::Boxed(a), Schema::Boxed(b)) => {
            return diff_schema(a, b, path, is_return_pos);
        }
        (Schema::Reference(a), Schema::Reference(b)) => {
            return diff_schema(a, b, path, is_return_pos);
        }
        (Schema::Slice(a), Schema::Slice(b)) => {
            return diff_schema(a, b, path, is_return_pos);
        }
        (Schema::Trait(amut, a), Schema::Trait(bmut, b)) | (Schema::FnClosure(amut, a), Schema::FnClosure(bmut, b)) => {
            if amut != bmut {
                if *amut {
                    return Some(format!(
                        "At location [{}]: Application protocol uses FnMut, but foreign format has Fn.",
                        path
                    ));
                }
                if *bmut {
                    return Some(format!(
                        "At location [{}]: Application protocol uses Fn, but foreign format uses FnMut.",
                        path
                    ));
                }
            }
            return diff_abi_def(a, b, path, is_return_pos);
        }
        (Schema::Recursion(adepth), Schema::Recursion(bdepth)) => {
            if adepth == bdepth {
                return None; //Ok
            } else {
                return Some(format!(
                    "At location [{}]: Application protocol uses recursion up {} levels, but foreign format uses {}.",
                    path, adepth, bdepth
                ));
            }
        }
        (Schema::Future(a, a_send, a_sync, a_unpin), Schema::Future(b, b_send, b_sync, b_unpin)) => {
            if !is_return_pos {
                panic!("Futures are only supported in return position");
            }
            for (a, b, bound) in [
                (*a_send, *b_send, "Send"),
                (*a_sync, *b_sync, "Sync"),
                (*a_unpin, *b_unpin, "Unpin"),
            ] {
                if a && !b {
                    return Some(format!(
                        "At location [{}]: Caller expects a future with an {}-bound, but implementation provides one without. This is an incompatible difference.",
                        path, bound
                    ));
                }
            }
            return diff_abi_def(a, b, path, is_return_pos);
        }
        (a, b) => (a.top_level_description(), b.top_level_description()),
    };

    Some(format!(
        "At location [{}]: In memory schema: {}, file schema: {}",
        path, atype, btype
    ))
}

fn diff_abi_def(a: &AbiTraitDefinition, b: &AbiTraitDefinition, path: String, is_return_pos: bool) -> Option<String> {
    for amet in a.methods.iter() {
        if let Some(bmet) = b.methods.iter().find(|x| x.name == amet.name) {
            if amet.info.arguments.len() != bmet.info.arguments.len() {
                return Some(format!(
                    "At location [{}]: Application protocol method {} has {} args, but foreign version has {}.",
                    path,
                    amet.name,
                    amet.info.arguments.len(),
                    bmet.info.arguments.len()
                ));
            }
            for (arg_index, (a_arg, b_arg)) in amet.info.arguments.iter().zip(bmet.info.arguments.iter()).enumerate() {
                if let Some(diff) = diff_schema(
                    &a_arg.schema,
                    &b_arg.schema,
                    format!("{}(arg #{})", amet.name, arg_index),
                    is_return_pos,
                ) {
                    return Some(diff);
                }
            }
        }
    }
    return None;
}

impl WithSchema for Field {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}

impl Serialize for Field {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.name)?;
        self.value.serialize(serializer)?;
        self.offset.serialize(serializer)?;
        Ok(())
    }
}
impl Packed for Field {}
impl Deserialize for Field {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Field {
            name: deserializer.read_string()?,
            value: Box::new(Schema::deserialize(deserializer)?),
            offset: if deserializer.file_version > 0 {
                Option::deserialize(deserializer)?
            } else {
                None
            },
        })
    }
}
impl WithSchema for Variant {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for Variant {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.name)?;
        serializer.write_u8(self.discriminant)?;
        serializer.write_usize_packed(self.fields.len())?;
        for field in &self.fields {
            field.serialize(serializer)?;
        }
        Ok(())
    }
}

impl Packed for Variant {}
impl Deserialize for Variant {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Variant {
            name: deserializer.read_string()?,
            discriminant: deserializer.read_u8()?,
            fields: {
                let l = deserializer.read_usize_packed()?;
                let mut ret = Vec::new();
                for _ in 0..l {
                    ret.push(Field {
                        name: deserializer.read_string()?,
                        value: Box::new(Schema::deserialize(deserializer)?),
                        offset: if deserializer.file_version > 0 {
                            Option::deserialize(deserializer)?
                        } else {
                            None
                        },
                    });
                }
                ret
            },
        })
    }
}
impl Serialize for SchemaArray {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize_packed(self.count)?;
        self.item_type.serialize(serializer)?;
        Ok(())
    }
}
impl Packed for SchemaArray {}
impl Deserialize for SchemaArray {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let count = deserializer.read_usize_packed()?;
        let item_type = Box::new(Schema::deserialize(deserializer)?);
        Ok(SchemaArray { count, item_type })
    }
}
impl WithSchema for SchemaArray {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}

impl WithSchema for SchemaStruct {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for SchemaStruct {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.dbg_name)?;
        serializer.write_usize_packed(self.fields.len())?;
        self.size.serialize(serializer)?;
        self.alignment.serialize(serializer)?;
        for field in &self.fields {
            field.serialize(serializer)?;
        }
        Ok(())
    }
}
impl Packed for SchemaStruct {}
impl Deserialize for SchemaStruct {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let dbg_name = deserializer.read_string()?;
        let l = deserializer.read_usize_packed()?;
        Ok(SchemaStruct {
            dbg_name,
            size: if deserializer.file_version > 0 {
                <_ as Deserialize>::deserialize(deserializer)?
            } else {
                None
            },
            alignment: if deserializer.file_version > 0 {
                <_ as Deserialize>::deserialize(deserializer)?
            } else {
                None
            },
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
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
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
            SchemaPrimitive::schema_f32 => 10,
            SchemaPrimitive::schema_f64 => 11,
            SchemaPrimitive::schema_bool => 12,
            SchemaPrimitive::schema_canary1 => 13,
            SchemaPrimitive::schema_i128 => 14,
            SchemaPrimitive::schema_u128 => 15,
            SchemaPrimitive::schema_char => 16,
            SchemaPrimitive::schema_string(layout) => {
                serializer.write_u8(9)?;
                if serializer.file_version > 0 {
                    serializer.write_u8(layout as u8)?;
                }
                return Ok(());
            }
        };
        serializer.write_u8(discr)
    }
}
impl WithSchema for VecOrStringLayout {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}
impl Deserialize for VecOrStringLayout {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(match deserializer.read_u8()? {
            1 => VecOrStringLayout::DataCapacityLength,
            2 => VecOrStringLayout::DataLengthCapacity,
            3 => VecOrStringLayout::CapacityDataLength,
            4 => VecOrStringLayout::LengthDataCapacity,
            5 => VecOrStringLayout::CapacityLengthData,
            6 => VecOrStringLayout::LengthCapacityData,
            7 => VecOrStringLayout::LengthData,
            8 => VecOrStringLayout::DataLength,
            _ => VecOrStringLayout::Unknown,
        })
    }
}
impl Packed for SchemaPrimitive {}
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
            9 => SchemaPrimitive::schema_string({
                if deserializer.file_version > 0 {
                    VecOrStringLayout::deserialize(deserializer)?
                } else {
                    VecOrStringLayout::Unknown
                }
            }),
            10 => SchemaPrimitive::schema_f32,
            11 => SchemaPrimitive::schema_f64,
            12 => SchemaPrimitive::schema_bool,
            13 => SchemaPrimitive::schema_canary1,
            14 => SchemaPrimitive::schema_i128,
            15 => SchemaPrimitive::schema_u128,
            16 => SchemaPrimitive::schema_char,
            c => {
                return Err(SavefileError::GeneralError {
                    msg: format!(
                        "Corrupt schema, type {} encountered. Perhaps data is from future version?",
                        c
                    ),
                })
            }
        };
        Ok(var)
    }
}

impl WithSchema for SchemaEnum {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}

impl Serialize for SchemaEnum {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(&self.dbg_name)?;
        serializer.write_usize_packed(self.variants.len())?;
        for var in &self.variants {
            var.serialize(serializer)?;
        }
        self.discriminant_size.serialize(serializer)?;
        self.has_explicit_repr.serialize(serializer)?;
        self.size.serialize(serializer)?;
        self.alignment.serialize(serializer)?;
        Ok(())
    }
}
impl Packed for SchemaEnum {}
impl Deserialize for SchemaEnum {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let dbg_name = deserializer.read_string()?;
        let l = deserializer.read_usize_packed()?;
        let mut ret = Vec::new();
        for _ in 0..l {
            ret.push(Variant::deserialize(deserializer)?);
        }
        let (discriminant_size, has_explicit_repr, size, alignment) = if deserializer.file_version > 0 {
            (
                u8::deserialize(deserializer)?,
                bool::deserialize(deserializer)?,
                Option::<usize>::deserialize(deserializer)?,
                Option::<usize>::deserialize(deserializer)?,
            )
        } else {
            (1, false, None, None)
        };
        Ok(SchemaEnum {
            dbg_name,
            variants: ret,
            discriminant_size,
            has_explicit_repr,
            size,
            alignment,
        })
    }
}

#[cfg(feature = "quickcheck")]
impl Arbitrary for VecOrStringLayout {
    fn arbitrary(g: &mut Gen) -> Self {
        let x = u8::arbitrary(g);
        match x % 9 {
            0 => VecOrStringLayout::Unknown,
            1 => VecOrStringLayout::DataCapacityLength,
            2 => VecOrStringLayout::DataLengthCapacity,
            3 => VecOrStringLayout::CapacityDataLength,
            4 => VecOrStringLayout::LengthDataCapacity,
            5 => VecOrStringLayout::CapacityLengthData,
            6 => VecOrStringLayout::LengthCapacityData,
            7 => VecOrStringLayout::LengthData,
            8 => VecOrStringLayout::DataLength,
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "quickcheck")]
impl Arbitrary for SchemaPrimitive {
    fn arbitrary(g: &mut Gen) -> Self {
        let x = u8::arbitrary(g);
        match x % 16 {
            0 => SchemaPrimitive::schema_i8,
            1 => SchemaPrimitive::schema_u8,
            2 => SchemaPrimitive::schema_i16,
            3 => SchemaPrimitive::schema_u16,
            4 => SchemaPrimitive::schema_i32,
            5 => SchemaPrimitive::schema_u32,
            6 => SchemaPrimitive::schema_i64,
            7 => SchemaPrimitive::schema_u64,
            8 => SchemaPrimitive::schema_string(VecOrStringLayout::arbitrary(g)),
            9 => SchemaPrimitive::schema_f32,
            10 => SchemaPrimitive::schema_f64,
            11 => SchemaPrimitive::schema_bool,
            12 => SchemaPrimitive::schema_canary1,
            13 => SchemaPrimitive::schema_u128,
            14 => SchemaPrimitive::schema_i128,
            15 => SchemaPrimitive::schema_char,
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "quickcheck")]
impl Arbitrary for Field {
    fn arbitrary(g: &mut Gen) -> Self {
        Field {
            name: g.choose(&["", "test"]).unwrap().to_string(),
            value: <_ as Arbitrary>::arbitrary(g),
            offset: <_ as Arbitrary>::arbitrary(g),
        }
    }
}

#[cfg(feature = "quickcheck")]
impl Arbitrary for Variant {
    fn arbitrary(g: &mut Gen) -> Self {
        Variant {
            name: g.choose(&["", "test"]).unwrap().to_string(),
            discriminant: <_ as Arbitrary>::arbitrary(g),
            fields: <_ as Arbitrary>::arbitrary(g),
        }
    }
}

#[cfg(feature = "quickcheck")]
impl Arbitrary for SchemaEnum {
    fn arbitrary(g: &mut Gen) -> Self {
        SchemaEnum {
            dbg_name: g.choose(&["", "test"]).unwrap().to_string(),
            variants: (0..*g.choose(&[0usize, 1, 2, 3]).unwrap())
                .map(|_| <_ as Arbitrary>::arbitrary(g))
                .collect(),
            discriminant_size: *g.choose(&[1, 2, 4]).unwrap(),
            has_explicit_repr: *g.choose(&[false, true]).unwrap(),
            size: <_ as Arbitrary>::arbitrary(g),
            alignment: <_ as Arbitrary>::arbitrary(g),
        }
    }
}

#[cfg(feature = "quickcheck")]
impl Arbitrary for SchemaStruct {
    fn arbitrary(g: &mut Gen) -> Self {
        SchemaStruct {
            fields: (0..*g.choose(&[0usize, 1, 2, 3]).unwrap())
                .map(|_| <_ as Arbitrary>::arbitrary(g))
                .collect(),
            dbg_name: <_ as Arbitrary>::arbitrary(g),
            size: <_ as Arbitrary>::arbitrary(g),
            alignment: <_ as Arbitrary>::arbitrary(g),
        }
    }
}
#[cfg(feature = "quickcheck")]
impl Arbitrary for SchemaArray {
    fn arbitrary(g: &mut Gen) -> Self {
        SchemaArray {
            item_type: <_ as Arbitrary>::arbitrary(g),
            count: <_ as Arbitrary>::arbitrary(g),
        }
    }
}
#[cfg(feature = "quickcheck")]
static QUICKCHECKBOUND: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "quickcheck")]
impl Arbitrary for Schema {
    fn arbitrary(g: &mut Gen) -> Self {
        let val = QUICKCHECKBOUND.fetch_add(1, Ordering::Relaxed);
        if val > 1 {
            QUICKCHECKBOUND.fetch_sub(1, Ordering::Relaxed);
            return Schema::ZeroSize;
        }
        let arg = g.choose(&[0, 1, 2, 3, 4, 5, 6, 7]).unwrap_or(&8);
        let temp = match arg {
            0 => Schema::Struct(<_ as Arbitrary>::arbitrary(g)),
            1 => Schema::Enum(<_ as Arbitrary>::arbitrary(g)),
            2 => Schema::Primitive(<_ as Arbitrary>::arbitrary(g)),
            3 => Schema::Vector(<_ as Arbitrary>::arbitrary(g), VecOrStringLayout::arbitrary(g)),
            4 => Schema::Array(SchemaArray::arbitrary(g)),
            5 => Schema::SchemaOption(<_ as Arbitrary>::arbitrary(g)),
            //Don't generate 'Undefined', since some of our tests assume not
            6 => Schema::ZeroSize,
            7 => Schema::Custom(g.choose(&["", "test"]).unwrap().to_string()),
            _ => Schema::ZeroSize,
        };
        _ = QUICKCHECKBOUND.fetch_sub(1, Ordering::Relaxed);
        temp
    }
}

impl WithSchema for Schema {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Undefined
    }
}
impl Serialize for Schema {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        match self {
            Schema::Struct(schema_struct) => {
                serializer.write_u8(1)?;
                schema_struct.serialize(serializer)
            }
            Schema::Enum(schema_enum) => {
                serializer.write_u8(2)?;
                schema_enum.serialize(serializer)
            }
            Schema::Primitive(schema_prim) => {
                serializer.write_u8(3)?;
                schema_prim.serialize(serializer)?;
                Ok(())
            }
            Schema::Vector(schema_vector, is_standard_layout) => {
                serializer.write_u8(4)?;
                schema_vector.serialize(serializer)?;
                if serializer.file_version > 0 {
                    serializer.write_u8(*is_standard_layout as u8)?;
                }
                Ok(())
            }
            Schema::Undefined => serializer.write_u8(5),
            Schema::ZeroSize => serializer.write_u8(6),
            Schema::SchemaOption(content) => {
                serializer.write_u8(7)?;
                content.serialize(serializer)
            }
            Schema::Array(array) => {
                serializer.write_u8(8)?;
                array.serialize(serializer)
            }
            Schema::Custom(custom) => {
                serializer.write_u8(9)?;
                custom.serialize(serializer)
            }
            Schema::Boxed(name) => {
                serializer.write_u8(10)?;
                name.serialize(serializer)
            }
            Schema::FnClosure(a, b) => {
                serializer.write_u8(11)?;
                a.serialize(serializer)?;
                b.serialize(serializer)?;
                Ok(())
            }
            Schema::Slice(inner) => {
                serializer.write_u8(12)?;
                inner.serialize(serializer)?;
                Ok(())
            }
            Schema::Str => {
                serializer.write_u8(13)?;
                Ok(())
            }
            Schema::Reference(inner) => {
                serializer.write_u8(14)?;
                inner.serialize(serializer)?;
                Ok(())
            }
            Schema::Trait(a, b) => {
                serializer.write_u8(15)?;
                serializer.write_bool(*a)?;
                b.serialize(serializer)?;
                Ok(())
            }
            Schema::Recursion(depth) => {
                serializer.write_u8(16)?;
                serializer.write_usize_packed(*depth)?;
                Ok(())
            }
            Schema::StdIoError => {
                serializer.write_u8(17)?;
                Ok(())
            }
            Schema::Future(o, send, sync, unpin) => {
                serializer.write_u8(18)?;
                serializer
                    .write_u8(if *send { 1 } else { 0 } | if *sync { 2 } else { 0 } | if *unpin { 4 } else { 0 })?;
                o.serialize(serializer)?;
                Ok(())
            }
        }
    }
}

impl Packed for Schema {}
impl Deserialize for Schema {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let x = deserializer.read_u8()?;
        let schema = match x {
            1 => Schema::Struct(SchemaStruct::deserialize(deserializer)?),
            2 => Schema::Enum(SchemaEnum::deserialize(deserializer)?),
            3 => Schema::Primitive(SchemaPrimitive::deserialize(deserializer)?),
            4 => Schema::Vector(
                Box::new(Schema::deserialize(deserializer)?),
                if deserializer.file_version > 0 {
                    VecOrStringLayout::deserialize(deserializer)?
                } else {
                    VecOrStringLayout::Unknown
                },
            ),
            5 => Schema::Undefined,
            6 => Schema::ZeroSize,
            7 => Schema::SchemaOption(Box::new(Schema::deserialize(deserializer)?)),
            8 => Schema::Array(SchemaArray::deserialize(deserializer)?),
            9 => Schema::Custom(String::deserialize(deserializer)?),
            10 => Schema::Boxed(<_ as Deserialize>::deserialize(deserializer)?),
            11 => Schema::FnClosure(
                <_ as Deserialize>::deserialize(deserializer)?,
                <_ as Deserialize>::deserialize(deserializer)?,
            ),
            12 => Schema::Slice(Box::new(<_ as Deserialize>::deserialize(deserializer)?)),
            13 => Schema::Str,
            14 => Schema::Reference(Box::new(<_ as Deserialize>::deserialize(deserializer)?)),
            15 => Schema::Trait(
                <_ as Deserialize>::deserialize(deserializer)?,
                <_ as Deserialize>::deserialize(deserializer)?,
            ),
            16 => Schema::Recursion(<_ as Deserialize>::deserialize(deserializer)?),
            17 => Schema::StdIoError,
            18 => {
                let mask = deserializer.read_u8()?;
                let send = (mask & 1) != 0;
                let sync = (mask & 2) != 0;
                let unpin = (mask & 4) != 0;
                Schema::Future(<_ as Deserialize>::deserialize(deserializer)?, send, sync, unpin)
            }
            c => {
                return Err(SavefileError::GeneralError {
                    msg: format!("Corrupt, or future schema, schema variant {} encountered", c),
                })
            }
        };

        Ok(schema)
    }
}
impl WithSchema for str {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string(VecOrStringLayout::Unknown))
    }
}

impl WithSchema for String {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string(calculate_string_memory_layout()))
    }
}
impl Introspect for str {
    fn introspect_value(&self) -> String {
        self.to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem>> {
        None
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
impl Serialize for str {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(self)
    }
}

impl Packed for String {}

impl Packed for str {}

impl Deserialize for String {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<String, SavefileError> {
        deserializer.read_string()
    }
}

/// Type of single child of introspector for Mutex
#[cfg(feature = "parking_lot")]
pub struct IntrospectItemMutex<'a, T> {
    g: MutexGuard<'a, T>,
}

#[cfg(feature = "parking_lot")]
impl<'a, T: Introspect> IntrospectItem<'a> for IntrospectItemMutex<'a, T> {
    fn key(&self) -> &str {
        "0"
    }

    fn val(&self) -> &dyn Introspect {
        self.g.deref()
    }
}

#[cfg(feature = "parking_lot")]
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
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        T::schema(version, context)
    }
}
impl<T> Packed for std::sync::Mutex<T> {}
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

#[cfg(feature = "parking_lot")]
impl<T: WithSchema> WithSchema for Mutex<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        T::schema(version, context)
    }
}

#[cfg(feature = "parking_lot")]
impl<T> Packed for Mutex<T> {}

#[cfg(feature = "parking_lot")]
impl<T: Serialize> Serialize for Mutex<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let data = self.lock();
        data.serialize(serializer)
    }
}

#[cfg(feature = "parking_lot")]
impl<T: Deserialize> Deserialize for Mutex<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Mutex<T>, SavefileError> {
        Ok(Mutex::new(T::deserialize(deserializer)?))
    }
}

/// Type of single child of introspector for RwLock
#[cfg(feature = "parking_lot")]
pub struct IntrospectItemRwLock<'a, T> {
    g: RwLockReadGuard<'a, T>,
}

#[cfg(feature = "parking_lot")]
impl<'a, T: Introspect> IntrospectItem<'a> for IntrospectItemRwLock<'a, T> {
    fn key(&self) -> &str {
        "0"
    }

    fn val(&self) -> &dyn Introspect {
        self.g.deref()
    }
}

impl<T: Introspect> Introspect for std::cell::Ref<'_, T> {
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

impl<'a, T: Introspect> IntrospectItem<'a> for std::cell::Ref<'a, T> {
    fn key(&self) -> &str {
        "ref"
    }
    /// The introspectable value of the child.
    fn val(&self) -> &dyn Introspect {
        self
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
#[cfg(feature = "parking_lot")]
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

#[cfg(feature = "parking_lot")]
impl<T: WithSchema> WithSchema for RwLock<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        T::schema(version, context)
    }
}

#[cfg(feature = "parking_lot")]
impl<T> Packed for RwLock<T> {}

#[cfg(feature = "parking_lot")]
impl<T: Serialize> Serialize for RwLock<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let data = self.read();
        data.serialize(serializer)
    }
}

#[cfg(feature = "parking_lot")]
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
    Box::new(IntrospectItemSimple { key, val })
}

#[cfg(not(feature = "nightly"))]
impl<K: Introspect + Eq + Hash, V: Introspect, S: ::std::hash::BuildHasher> Introspect for HashMap<K, V, S> {
    fn introspect_value(&self) -> String {
        format!("HashMap<{},{}>", std::any::type_name::<K>(), std::any::type_name::<V>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((key, val)) = self.iter().nth(bucket) {
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
        if let Some((key, val)) = self.iter().nth(bucket) {
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
        if let Some((key, val)) = self.iter().nth(index) {
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
        if let Some(key) = self.iter().nth(index) {
            Some(introspect_item(format!("#{}", index), key))
        } else {
            None
        }
    }
    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<K: Introspect> Introspect for BTreeSet<K> {
    fn introspect_value(&self) -> String {
        format!("BTreeSet<{}>", std::any::type_name::<K>())
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        if let Some(key) = self.iter().nth(index) {
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
        format!(
            "BTreeMap<{},{}>",
            std::any::type_name::<K>(),
            std::any::type_name::<V>()
        )
    }

    // This has very bad performance. But with the model behind Savefile Introspect it
    // is presently hard to do much better
    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        let bucket = index / 2;
        let off = index % 2;
        if let Some((key, val)) = self.iter().nth(bucket) {
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
impl<K: WithSchema + 'static, V: WithSchema + 'static> WithSchema for BTreeMap<K, V> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(Schema::Struct(SchemaStruct {
                dbg_name: "KeyValuePair".to_string(),
                size: None,
                alignment: None,
                fields: vec![
                    Field {
                        name: "key".to_string(),
                        value: Box::new(context.possible_recursion::<K>(|context| K::schema(version, context))),
                        offset: None,
                    },
                    Field {
                        name: "value".to_string(),
                        value: Box::new(context.possible_recursion::<V>(|context| V::schema(version, context))),
                        offset: None,
                    },
                ],
            })),
            VecOrStringLayout::Unknown,
        )
    }
}
impl<K, V> Packed for BTreeMap<K, V> {}
impl<K: Serialize + 'static, V: Serialize + 'static> Serialize for BTreeMap<K, V> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.len().serialize(serializer)?;
        for (k, v) in self {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}
impl<K: Deserialize + Ord + 'static, V: Deserialize + 'static> Deserialize for BTreeMap<K, V> {
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

impl<K> Packed for BTreeSet<K> {}
impl<K: WithSchema + 'static> WithSchema for BTreeSet<K> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<K>(|context| K::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
    }
}
impl<K: Serialize + 'static> Serialize for BTreeSet<K> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize_packed(self.len())?;
        for item in self {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}
impl<K: Deserialize + 'static + Ord> Deserialize for BTreeSet<K> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let cnt = deserializer.read_usize_packed()?;
        let mut ret = BTreeSet::new();
        for _ in 0..cnt {
            ret.insert(<_ as Deserialize>::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

impl<K, S: ::std::hash::BuildHasher> Packed for HashSet<K, S> {}
impl<K: WithSchema + 'static, S: ::std::hash::BuildHasher> WithSchema for HashSet<K, S> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<K>(|context| K::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
    }
}
impl<K: Serialize + 'static, S: ::std::hash::BuildHasher> Serialize for HashSet<K, S> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize_packed(self.len())?;
        for item in self {
            item.serialize(serializer)?;
        }
        Ok(())
    }
}
impl<K: Deserialize + Eq + Hash + 'static, S: ::std::hash::BuildHasher + Default> Deserialize for HashSet<K, S> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let cnt = deserializer.read_usize_packed()?;
        let mut ret = HashSet::with_capacity_and_hasher(cnt, S::default());
        for _ in 0..cnt {
            ret.insert(<_ as Deserialize>::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

impl<K: WithSchema + Eq + Hash + 'static, V: WithSchema + 'static, S: ::std::hash::BuildHasher> WithSchema
    for HashMap<K, V, S>
{
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(Schema::Struct(SchemaStruct {
                dbg_name: "KeyValuePair".to_string(),
                size: None,
                alignment: None,
                fields: vec![
                    Field {
                        name: "key".to_string(),
                        value: Box::new(context.possible_recursion::<K>(|context| K::schema(version, context))),
                        offset: None,
                    },
                    Field {
                        name: "value".to_string(),
                        value: Box::new(context.possible_recursion::<K>(|context| V::schema(version, context))),
                        offset: None,
                    },
                ],
            })),
            VecOrStringLayout::Unknown,
        )
    }
}
impl<K: Eq + Hash, V, S: ::std::hash::BuildHasher> Packed for HashMap<K, V, S> {}
impl<K: Serialize + Eq + Hash + 'static, V: Serialize + 'static, S: ::std::hash::BuildHasher> Serialize
    for HashMap<K, V, S>
{
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize_packed(self.len())?;
        for (k, v) in self.iter() {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}

impl<K: Deserialize + Eq + Hash + 'static, V: Deserialize + 'static, S: ::std::hash::BuildHasher + Default> Deserialize
    for HashMap<K, V, S>
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize_packed()?;
        let mut ret: Self = HashMap::with_capacity_and_hasher(l, Default::default());
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

#[cfg(feature = "indexmap")]
impl<K: WithSchema + Eq + Hash + 'static, V: WithSchema + 'static, S: ::std::hash::BuildHasher> WithSchema
    for IndexMap<K, V, S>
{
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(Schema::Struct(SchemaStruct {
                dbg_name: "KeyValuePair".to_string(),
                size: None,
                alignment: None,
                fields: vec![
                    Field {
                        name: "key".to_string(),
                        value: Box::new(context.possible_recursion::<K>(|context| K::schema(version, context))),
                        offset: None,
                    },
                    Field {
                        name: "value".to_string(),
                        value: Box::new(context.possible_recursion::<K>(|context| V::schema(version, context))),
                        offset: None,
                    },
                ],
            })),
            VecOrStringLayout::Unknown,
        )
    }
}

#[cfg(all(not(feature = "nightly"), feature = "indexmap"))]
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

#[cfg(all(feature = "nightly", feature = "indexmap"))]
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

#[cfg(all(feature = "nightly", feature = "indexmap"))]
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
#[cfg(feature = "indexmap")]
impl<K: Eq + Hash, V, S: ::std::hash::BuildHasher> Packed for IndexMap<K, V, S> {}

#[cfg(feature = "indexmap")]
impl<K: Serialize + Eq + Hash + 'static, V: Serialize + 'static, S: ::std::hash::BuildHasher> Serialize
    for IndexMap<K, V, S>
{
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize_packed(self.len())?;
        for (k, v) in self.iter() {
            k.serialize(serializer)?;
            v.serialize(serializer)?;
        }
        Ok(())
    }
}

#[cfg(feature = "indexmap")]
impl<K: Deserialize + Eq + Hash + 'static, V: Deserialize + 'static> Deserialize for IndexMap<K, V> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize_packed()?;
        let mut ret = IndexMap::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

#[cfg(feature = "indexmap")]
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

#[cfg(feature = "indexmap")]
impl<K: Eq + Hash, S: ::std::hash::BuildHasher> Packed for IndexSet<K, S> {}

#[cfg(feature = "indexmap")]
impl<K: WithSchema + Eq + Hash + 'static, S: ::std::hash::BuildHasher> WithSchema for IndexSet<K, S> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(Schema::Struct(SchemaStruct {
                dbg_name: "Key".to_string(),
                size: None,
                alignment: None,
                fields: vec![Field {
                    name: "key".to_string(),
                    value: Box::new(context.possible_recursion::<K>(|context| K::schema(version, context))),
                    offset: None,
                }],
            })),
            VecOrStringLayout::Unknown,
        )
    }
}

#[cfg(feature = "indexmap")]
impl<K: Serialize + Eq + Hash + 'static, S: ::std::hash::BuildHasher> Serialize for IndexSet<K, S> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_usize_packed(self.len())?;
        for k in self.iter() {
            k.serialize(serializer)?;
        }
        Ok(())
    }
}

#[cfg(feature = "indexmap")]
impl<K: Deserialize + Eq + Hash + 'static> Deserialize for IndexSet<K> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize_packed()?;
        let mut ret = IndexSet::with_capacity(l);
        for _ in 0..l {
            ret.insert(K::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

/// Something that can construct a value of type T.
/// Used when a field has been removed using the `AbiRemoved` type.
/// Usage:
/// ```rust
/// use savefile::{AbiRemoved, ValueConstructor};
/// use savefile_derive::Savefile;
/// #[derive(Savefile)]
/// struct MyStruct {
///     my_field: String,
///     #[savefile_versions="..0"]
///     my_removed_field: AbiRemoved<String, MyStructMyRemovedFieldFactory>,
/// }
/// struct MyStructMyRemovedFieldFactory;
/// impl ValueConstructor<String> for MyStructMyRemovedFieldFactory {
///     fn make_value() -> String {
///         "Default value for when values of version 0 are to be serialized".to_string()
///     }
/// }
/// ```
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot serve as a factory generating default values of type {T}, since it doesn't implement the trait `savefile::ValueConstructor<{T}>`-",
        label = "`{Self}` cannot produce values of type `{T}`",
        note = "Check that any type used as 2nd type parameter to AbiRemoved implements `savefile::ValueConstructor<{T}>`.",
        note = "Alternatively, skip the 2nd parameter entirely, and ensure that `{T}` implements `Default`.",
    )
)]
pub trait ValueConstructor<T> {
    /// Create a value of type T.
    /// This is used by the AbiRemoved trait to be able to invent
    /// values when writing removed fields from old protocols.
    fn make_value() -> T;
}

/// A value constructor that delegates to the 'Default' trait.
/// Requires that type `T` implements `Default`.
#[derive(Debug, PartialEq, Eq)]
pub struct DefaultValueConstructor<T> {
    phantom: PhantomData<*const T>,
}

impl<T: Default> ValueConstructor<T> for DefaultValueConstructor<T> {
    fn make_value() -> T {
        <T as Default>::default()
    }
}

/// Helper struct which represents a field which has been removed.
///
/// In contrast to AbiRemoved, this type only supports deserialization.
/// It is thus not recommended for use when SavefileAbi is to be used, and
/// forward compatibility is desired.
///
/// The difference is that Removed does not require T to implement Default,
/// or any other factory trait, since we never need to serialize dummy
/// values of Removed (we never serialize using a schema where a field i Removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Removed<T> {
    phantom: std::marker::PhantomData<*const T>,
}

/// Removed is a zero-sized type. It contains a PhantomData<*const T>, which means
/// it doesn't implement Send or Sync per default. However, implementing these
/// is actually safe, so implement it manually.
unsafe impl<T> Send for Removed<T> {}
/// Removed is a zero-sized type. It contains a PhantomData<*const T>, which means
/// it doesn't implement Send or Sync per default. However, implementing these
/// is actually safe, so implement it manually.
unsafe impl<T> Sync for Removed<T> {}

impl<T> Removed<T> {
    /// Helper to create an instance of `Removed<T>`. `Removed<T>` has no data.
    pub fn new() -> Removed<T> {
        Removed {
            phantom: std::marker::PhantomData,
        }
    }
}
impl<T: WithSchema> WithSchema for Removed<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        <T>::schema(version, context)
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
impl<T> Packed for Removed<T> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
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

/// Helper struct which represents a field which has been removed, for use with
/// SavefileAbi - supporting both serialization and deserialization.
///
/// In contrast to `Removed`, this type supports both serialization and
/// deserialization, and is preferred when SavefileAbi is to be used.
/// Regular Savefile does not support serializing older versions, whereas
/// SavefileAbi does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiRemoved<T, D = DefaultValueConstructor<T>>
where
    D: ValueConstructor<T>,
{
    phantom: std::marker::PhantomData<(*const T, *const D)>,
}

/// Removed is a zero-sized type. It contains a PhantomData<*const T>, which means
/// it doesn't implement Send or Sync per default. However, implementing these
/// is actually safe, so implement it manually.
unsafe impl<T, D: ValueConstructor<T>> Send for AbiRemoved<T, D> {}
/// Removed is a zero-sized type. It contains a PhantomData<*const T>, which means
/// it doesn't implement Send or Sync per default. However, implementing these
/// is actually safe, so implement it manually.
unsafe impl<T, D: ValueConstructor<T>> Sync for AbiRemoved<T, D> {}

impl<T, D: ValueConstructor<T>> AbiRemoved<T, D> {
    /// Helper to create an instance of `AbiRemoved<T>`. `AbiRemoved<T>` has no data.
    pub fn new() -> AbiRemoved<T, D> {
        AbiRemoved {
            phantom: std::marker::PhantomData,
        }
    }
}

impl<T: WithSchema, D: ValueConstructor<T>> WithSchema for AbiRemoved<T, D> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        <T>::schema(version, context)
    }
}

impl<T: Introspect, D: ValueConstructor<T>> Introspect for AbiRemoved<T, D> {
    fn introspect_value(&self) -> String {
        format!("AbiRemoved<{}>", std::any::type_name::<T>())
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}
impl<T, D: ValueConstructor<T>> Packed for AbiRemoved<T, D> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl<T: WithSchema + Serialize + Default, D: ValueConstructor<T>> Serialize for AbiRemoved<T, D> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let dummy = D::make_value();
        dummy.serialize(serializer)?;
        Ok(())
    }
}
impl<T: WithSchema + Deserialize, D: ValueConstructor<T>> Deserialize for AbiRemoved<T, D> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        T::deserialize(deserializer)?;
        Ok(AbiRemoved {
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
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::ZeroSize
    }
}
impl<T> Packed for std::marker::PhantomData<T> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
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
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::SchemaOption(Box::new(T::schema(version, context)))
    }
}
impl<T> Packed for Option<T> {} //Sadly, Option does not allow the #"reprC"-optimization
impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        match self {
            Some(ref x) => {
                serializer.write_bool(true)?;
                x.serialize(serializer)
            }
            None => serializer.write_bool(false),
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

impl<T: Introspect, R: Introspect> Introspect for Result<T, R> {
    fn introspect_value(&self) -> String {
        match self {
            Ok(cont) => format!("Ok({})", cont.introspect_value()),
            Err(cont) => format!("Err({})", cont.introspect_value()),
        }
    }

    fn introspect_child(&self, index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        match self {
            Ok(cont) => cont.introspect_child(index),
            Err(cont) => cont.introspect_child(index),
        }
    }
    fn introspect_len(&self) -> usize {
        match self {
            Ok(cont) => cont.introspect_len(),
            Err(cont) => cont.introspect_len(),
        }
    }
}

impl<T: WithSchema, R: WithSchema> WithSchema for Result<T, R> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Enum(SchemaEnum {
            dbg_name: "Result".to_string(),
            size: None,
            alignment: None,
            variants: vec![
                Variant {
                    name: "Ok".to_string(),
                    discriminant: 0,
                    fields: vec![Field {
                        name: "ok".to_string(),
                        value: Box::new(T::schema(version, context)),
                        offset: None,
                    }],
                },
                Variant {
                    name: "Err".to_string(),
                    discriminant: 0,
                    fields: vec![Field {
                        name: "err".to_string(),
                        value: Box::new(R::schema(version, context)),
                        offset: None,
                    }],
                },
            ],
            discriminant_size: 1,
            has_explicit_repr: false,
        })
    }
}
impl<T, R> Packed for Result<T, R> {} //Sadly, Result does not allow the #"reprC"-optimization
impl<T: Serialize, R: Serialize> Serialize for Result<T, R> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        match self {
            Ok(x) => {
                serializer.write_bool(true)?;
                x.serialize(serializer)
            }
            Err(x) => {
                serializer.write_bool(false)?;
                x.serialize(serializer)
            }
        }
    }
}
impl<T: Deserialize, R: Deserialize> Deserialize for Result<T, R> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let issome = deserializer.read_bool()?;
        if issome {
            Ok(Ok(T::deserialize(deserializer)?))
        } else {
            Ok(Err(R::deserialize(deserializer)?))
        }
    }
}

#[cfg(any(feature = "bit-vec", feature = "bit-vec08"))]
#[cfg(target_endian = "big")]
compile_error!("savefile bit-vec feature does not support big-endian machines");

#[cfg(feature = "bit-vec")]
impl WithSchema for bit_vec::BitVec {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "BitVec".to_string(),
            size: None,
            alignment: None,
            fields: vec![
                Field {
                    name: "num_bits".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "num_bytes".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "buffer".to_string(),
                    value: Box::new(Schema::Vector(
                        Box::new(u8::schema(version, context)),
                        VecOrStringLayout::Unknown,
                    )),
                    offset: None,
                },
            ],
        })
    }
}

#[cfg(feature = "bit-vec")]
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

#[cfg(feature = "bit-vec")]
impl Serialize for bit_vec::BitVec<u32> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize_packed(l)?;
        let storage = self.storage();
        let rawbytes_ptr = storage.as_ptr() as *const u8;
        let rawbytes: &[u8] = unsafe { std::slice::from_raw_parts(rawbytes_ptr, 4 * storage.len()) };
        serializer.write_usize(rawbytes.len() | (1 << 63))?;
        serializer.write_bytes(rawbytes)?;
        Ok(())
    }
}

#[cfg(feature = "bit-vec")]
impl Packed for bit_vec::BitVec<u32> {}

#[cfg(feature = "bit-vec")]
impl Deserialize for bit_vec::BitVec<u32> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let numbits = deserializer.read_usize_packed()?;
        let mut numbytes = deserializer.read_usize()?;
        if numbytes & (1 << 63) != 0 {
            //New format
            numbytes &= !(1 << 63);
            let mut ret = bit_vec::BitVec::with_capacity(numbytes * 8);
            unsafe {
                let num_words = numbytes / 4;
                let storage = ret.storage_mut();
                storage.resize(num_words, 0);
                let storage_ptr = storage.as_ptr() as *mut u8;
                let storage_bytes: &mut [u8] = std::slice::from_raw_parts_mut(storage_ptr, 4 * num_words);
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

#[cfg(feature = "bit-set")]
impl WithSchema for bit_set::BitSet {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "BitSet".to_string(),
            size: None,
            alignment: None,
            fields: vec![
                Field {
                    name: "num_bits".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "num_bytes".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "buffer".to_string(),
                    value: Box::new(Schema::Vector(
                        Box::new(u8::schema(version, context)),
                        VecOrStringLayout::Unknown,
                    )),
                    offset: None,
                },
            ],
        })
    }
}

#[cfg(feature = "bit-set")]
impl Introspect for bit_set::BitSet {
    fn introspect_value(&self) -> String {
        let mut ret = String::new();
        for i in 0..self.len() {
            if self.contains(i) {
                use std::fmt::Write;
                if !ret.is_empty() {
                    ret += " ";
                }
                write!(&mut ret, "{}", i).unwrap();
            }
        }
        ret
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

#[cfg(feature = "bit-set")]
impl Packed for bit_set::BitSet<u32> {}

#[cfg(feature = "bit-set")]
impl Serialize for bit_set::BitSet<u32> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let bitset = self.get_ref();
        bitset.serialize(serializer)
    }
}

#[cfg(feature = "bit-set")]
impl Deserialize for bit_set::BitSet<u32> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let bit_vec: bit_vec::BitVec = bit_vec::BitVec::deserialize(deserializer)?;
        Ok(bit_set::BitSet::from_bit_vec(bit_vec))
    }
}

#[cfg(feature = "bit-vec08")]
impl WithSchema for bit_vec08::BitVec {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "BitVec".to_string(),
            size: None,
            alignment: None,
            fields: vec![
                Field {
                    name: "num_bits".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "num_bytes".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "buffer".to_string(),
                    value: Box::new(Schema::Vector(
                        Box::new(u8::schema(version, context)),
                        VecOrStringLayout::Unknown,
                    )),
                    offset: None,
                },
            ],
        })
    }
}

#[cfg(feature = "bit-vec08")]
impl Introspect for bit_vec08::BitVec {
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

#[cfg(feature = "bit-vec08")]
impl Serialize for bit_vec08::BitVec<u32> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize_packed(l)?;
        let storage = self.storage();
        let rawbytes_ptr = storage.as_ptr() as *const u8;
        let rawbytes: &[u8] = unsafe { std::slice::from_raw_parts(rawbytes_ptr, 4 * storage.len()) };
        serializer.write_usize(rawbytes.len() | (1 << 63))?;
        serializer.write_bytes(rawbytes)?;
        Ok(())
    }
}

#[cfg(feature = "bit-vec08")]
impl Packed for bit_vec08::BitVec<u32> {}

#[cfg(feature = "bit-vec08")]
impl Deserialize for bit_vec08::BitVec<u32> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let numbits = deserializer.read_usize_packed()?;
        let mut numbytes = deserializer.read_usize()?;
        if numbytes & (1 << 63) != 0 {
            //New format
            numbytes &= !(1 << 63);
            let mut ret = bit_vec08::BitVec::with_capacity(numbytes * 8);
            unsafe {
                let num_words = numbytes / 4;
                let storage = ret.storage_mut();
                storage.resize(num_words, 0);
                let storage_ptr = storage.as_ptr() as *mut u8;
                let storage_bytes: &mut [u8] = std::slice::from_raw_parts_mut(storage_ptr, 4 * num_words);
                deserializer.read_bytes_to_buf(storage_bytes)?;
                ret.set_len(numbits);
            }
            Ok(ret)
        } else {
            let bytes = deserializer.read_bytes(numbytes)?;
            let mut ret = bit_vec08::BitVec::from_bytes(&bytes);
            ret.truncate(numbits);
            Ok(ret)
        }
    }
}

#[cfg(feature = "bit-set")]
impl WithSchema for bit_set08::BitSet {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "BitSet".to_string(),
            size: None,
            alignment: None,
            fields: vec![
                Field {
                    name: "num_bits".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "num_bytes".to_string(),
                    value: Box::new(usize::schema(version, context)),
                    offset: None,
                },
                Field {
                    name: "buffer".to_string(),
                    value: Box::new(Schema::Vector(
                        Box::new(u8::schema(version, context)),
                        VecOrStringLayout::Unknown,
                    )),
                    offset: None,
                },
            ],
        })
    }
}

#[cfg(feature = "bit-set08")]
impl Introspect for bit_set08::BitSet {
    fn introspect_value(&self) -> String {
        let mut ret = String::new();
        for i in 0..self.len() {
            if self.contains(i) {
                use std::fmt::Write;
                if !ret.is_empty() {
                    ret += " ";
                }
                write!(&mut ret, "{}", i).unwrap();
            }
        }
        ret
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem + '_>> {
        None
    }
}

#[cfg(feature = "bit-set08")]
impl Packed for bit_set08::BitSet<u32> {}

#[cfg(feature = "bit-set08")]
impl Serialize for bit_set08::BitSet<u32> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let bitset = self.get_ref();
        bitset.serialize(serializer)
    }
}

#[cfg(feature = "bit-set")]
impl Deserialize for bit_set08::BitSet<u32> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let bit_vec: bit_vec08::BitVec = bit_vec08::BitVec::deserialize(deserializer)?;
        Ok(bit_set08::BitSet::from_bit_vec(bit_vec))
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
        return Some(introspect_item(index.to_string(), self.iter().nth(index).unwrap()));
    }

    fn introspect_len(&self) -> usize {
        self.len()
    }
}

impl<T> Packed for BinaryHeap<T> {}
impl<T: WithSchema + 'static> WithSchema for BinaryHeap<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
    }
}
impl<T: Serialize + Ord + 'static> Serialize for BinaryHeap<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize_packed(l)?;
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())
    }
}
impl<T: Deserialize + Ord + 'static> Deserialize for BinaryHeap<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize_packed()?;
        let mut ret = BinaryHeap::with_capacity(l);
        for _ in 0..l {
            ret.push(T::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

#[cfg(feature = "smallvec")]
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

#[cfg(feature = "smallvec")]
impl<T: smallvec::Array + 'static> WithSchema for smallvec::SmallVec<T>
where
    T::Item: WithSchema,
{
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::Item::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
    }
}
#[cfg(feature = "smallvec")]
impl<T: smallvec::Array> Packed for smallvec::SmallVec<T> {}

#[cfg(feature = "smallvec")]
impl<T: smallvec::Array + 'static> Serialize for smallvec::SmallVec<T>
where
    T::Item: Serialize,
{
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize_packed(l)?;
        for item in self.iter() {
            item.serialize(serializer)?
        }
        Ok(())
    }
}
#[cfg(feature = "smallvec")]
impl<T: smallvec::Array + 'static> Deserialize for smallvec::SmallVec<T>
where
    T::Item: Deserialize,
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize_packed()?;
        let mut ret = Self::with_capacity(l);
        for _ in 0..l {
            ret.push(T::Item::deserialize(deserializer)?);
        }
        Ok(ret)
    }
}

fn regular_serialize_vec<T: Serialize>(
    items: &[T],
    serializer: &mut Serializer<impl Write>,
) -> Result<(), SavefileError> {
    let l = items.len();
    serializer.write_usize_packed(l)?;
    if std::mem::size_of::<T>() == 0 {
        return Ok(());
    }

    if std::mem::size_of::<T>() < 32 {
        //<-- This optimization seems to help a little actually, but maybe not enough to warrant it
        let chunks = items.chunks_exact((64 / std::mem::size_of::<T>()).max(1));
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

impl<T: WithSchema + 'static> WithSchema for Box<[T]> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
    }
}
impl<T: WithSchema + 'static> WithSchema for Arc<[T]> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
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
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string(VecOrStringLayout::Unknown))
    }
}
impl Introspect for Arc<str> {
    fn introspect_value(&self) -> String {
        self.deref().to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem>> {
        None
    }
    fn introspect_len(&self) -> usize {
        0
    }
}
impl Serialize for Arc<str> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(self)
    }
}

impl Packed for Arc<str> {}

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

impl<T: Serialize + Packed + 'static> Serialize for Box<[T]> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.file_version).is_false() {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize_packed(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    (*self).as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}
impl<T: Packed> Packed for Box<[T]> {}

impl<T: Serialize + Packed + 'static> Serialize for Arc<[T]> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.file_version).is_false() {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize_packed(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    (*self).as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}
impl<T: Packed> Packed for Arc<[T]> {}

impl<T: Deserialize + Packed + 'static> Deserialize for Arc<[T]> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Vec::<T>::deserialize(deserializer)?.into())
    }
}
impl<T: Deserialize + Packed + 'static> Deserialize for Box<[T]> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Vec::<T>::deserialize(deserializer)?.into_boxed_slice())
    }
}
impl WithSchema for &'_ str {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string(calculate_string_memory_layout()))
        //TODO: This is _not_ the same memory layout as vec. Make a new Box type for slices?
    }
}
impl Serialize for &'_ str {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        let l = self.len();
        serializer.write_usize_packed(l)?;
        serializer.write_buf(self.as_bytes())
    }
}

impl<T: WithSchema + 'static> WithSchema for &'_ [T] {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
            calculate_slice_memory_layout::<T>(),
        )
        //TODO: This is _not_ the same memory layout as vec. Make a new Box type for slices?
    }
}
impl<T: Serialize + Packed + 'static> Serialize for &'_ [T] {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.file_version).is_false() {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize_packed(l)?;
                #[allow(clippy::manual_slice_size_calculation)] // I feel this way is clearer
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}

/// Deserialize a slice into a Vec
/// Unsized slices cannot be deserialized into unsized slices.
pub fn deserialize_slice_as_vec<R: Read, T: Deserialize + Packed + 'static>(
    deserializer: &mut Deserializer<R>,
) -> Result<Vec<T>, SavefileError> {
    Vec::deserialize(deserializer)
}

impl<T> Packed for Vec<T> {}

/// 0 = Uninitialized
static STRING_IS_STANDARD_LAYOUT: AtomicU8 = AtomicU8::new(255);
#[derive(Debug)]
#[repr(C)]
struct RawVecInspector {
    p1: usize,
    p2: usize,
    p3: usize,
}
#[derive(Debug)]
#[repr(C)]
struct RawSliceInspector {
    p1: usize,
    p2: usize,
}
impl RawSliceInspector {
    const fn get_layout(&self) -> VecOrStringLayout {
        if self.p1 == 0 {
            VecOrStringLayout::LengthData
        } else {
            VecOrStringLayout::DataLength
        }
    }
}
impl RawVecInspector {
    fn get_layout(&self, ptr: *const u8) -> VecOrStringLayout {
        let ptr = ptr as usize;
        // We know size is 1, and capacity is 2.
        const LENGTH: usize = 0;
        const CAPACITY: usize = 7;
        match (self.p1, self.p2, self.p3) {
            (LENGTH, CAPACITY, x) if x == ptr => VecOrStringLayout::LengthCapacityData,
            (CAPACITY, LENGTH, x) if x == ptr => VecOrStringLayout::CapacityLengthData,
            (LENGTH, x, CAPACITY) if x == ptr => VecOrStringLayout::LengthDataCapacity,
            (CAPACITY, x, LENGTH) if x == ptr => VecOrStringLayout::CapacityDataLength,
            (x, LENGTH, CAPACITY) if x == ptr => VecOrStringLayout::DataLengthCapacity,
            (x, CAPACITY, LENGTH) if x == ptr => VecOrStringLayout::DataCapacityLength,
            _ => VecOrStringLayout::Unknown,
        }
    }
}

/// Calculate the memory layout of `&[T]`.
///
/// I.e, of the reference to the data.
/// This type is typically 16 bytes, consisting of two words, one being the length,
/// the other being a pointer to the start of the data.
pub const fn calculate_slice_memory_layout<T>() -> VecOrStringLayout {
    if std::mem::size_of::<&[T]>() != 16 || std::mem::size_of::<RawSliceInspector>() != 16 {
        VecOrStringLayout::Unknown
    } else {
        let test_slice: &[T] = &[];
        let insp: RawSliceInspector = unsafe { std::mem::transmute_copy::<&[T], RawSliceInspector>(&test_slice) };
        insp.get_layout()
    }
}
/// Calculate the memory layout of a Vec of the given type
pub fn calculate_vec_memory_layout<T>() -> VecOrStringLayout {
    if std::mem::size_of::<Vec<u8>>() != 24 || std::mem::size_of::<RawVecInspector>() != 24 {
        VecOrStringLayout::Unknown
    } else {
        let test_vec = Vec::with_capacity(7);
        let insp: RawVecInspector = unsafe { std::mem::transmute_copy(&test_vec) };
        let ptr = test_vec.as_ptr();
        insp.get_layout(ptr)
    }
}
fn calculate_string_memory_layout() -> VecOrStringLayout {
    let mut is_std = STRING_IS_STANDARD_LAYOUT.load(Ordering::Relaxed);
    if is_std != 255 {
        // SAFETY
        // We 'is_std' is always initialized using a valid VecOrStringLayout enum value,
        // unless it's 255.
        return unsafe { std::mem::transmute::<u8, VecOrStringLayout>(is_std) };
    }
    if std::mem::size_of::<String>() != 24 || std::mem::size_of::<RawVecInspector>() != 24 {
        is_std = VecOrStringLayout::Unknown as u8;
    } else {
        let test_string = String::with_capacity(7);
        let insp: RawVecInspector = unsafe { std::mem::transmute_copy(&test_string) };
        let ptr = test_string.as_ptr();

        is_std = insp.get_layout(ptr) as u8;

        drop(test_string);
    }

    STRING_IS_STANDARD_LAYOUT.store(is_std, Ordering::Relaxed);
    return unsafe { std::mem::transmute::<u8, VecOrStringLayout>(is_std) };
}
impl<T: WithSchema + 'static> WithSchema for Vec<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
            calculate_vec_memory_layout::<T>(),
        )
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

impl<T: Serialize + Packed + 'static> Serialize for Vec<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if T::repr_c_optimization_safe(serializer.file_version).is_false() {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize_packed(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<T>() * l,
                ))
            }
        }
    }
}

fn regular_deserialize_vec<T: Deserialize>(
    deserializer: &mut Deserializer<impl Read>,
) -> Result<Vec<T>, SavefileError> {
    let l = deserializer.read_usize_packed()?;

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

impl<T: Deserialize + Packed + 'static> Deserialize for Vec<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        if unsafe { T::repr_c_optimization_safe(deserializer.file_version) }.is_false() {
            Ok(regular_deserialize_vec(deserializer)?)
        } else {
            use std::mem;

            let align = mem::align_of::<T>();
            let elem_size = mem::size_of::<T>();
            let num_elems = deserializer.read_usize_packed()?;

            if num_elems == 0 {
                return Ok(Vec::new());
            }
            let num_bytes = elem_size * num_elems;

            let layout = if let Ok(layout) = std::alloc::Layout::from_size_align(num_bytes, align) {
                Ok(layout)
            } else {
                Err(SavefileError::MemoryAllocationLayoutError)
            }?;
            let ptr = if elem_size == 0 {
                NonNull::dangling().as_ptr()
            } else {
                let ptr = unsafe { std::alloc::alloc(layout) };
                if ptr.is_null() {
                    panic!("Failed to allocate {} bytes of memory", num_bytes);
                }

                ptr
            };

            {
                let slice = unsafe { std::slice::from_raw_parts_mut(ptr, num_bytes) };
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

impl<T: WithSchema + 'static> WithSchema for VecDeque<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(
            Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
            VecOrStringLayout::Unknown,
        )
    }
}

impl<T> Packed for VecDeque<T> {}
impl<T: Serialize + 'static> Serialize for VecDeque<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        regular_serialize_vecdeque(self, serializer)
    }
}

impl<T: Deserialize + 'static> Deserialize for VecDeque<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(regular_deserialize_vecdeque(deserializer)?)
    }
}

fn regular_serialize_vecdeque<T: Serialize>(
    item: &VecDeque<T>,
    serializer: &mut Serializer<impl Write>,
) -> Result<(), SavefileError> {
    let l = item.len();
    serializer.write_usize_packed(l)?;
    for item in item.iter() {
        item.serialize(serializer)?
    }
    Ok(())
}

fn regular_deserialize_vecdeque<T: Deserialize>(
    deserializer: &mut Deserializer<impl Read>,
) -> Result<VecDeque<T>, SavefileError> {
    let l = deserializer.read_usize_packed()?;
    let mut ret = VecDeque::with_capacity(l);
    for _ in 0..l {
        ret.push_back(T::deserialize(deserializer)?);
    }
    Ok(ret)
}

impl Packed for bool {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
} //It isn't really guaranteed that bool is an u8 or i8 where false = 0 and true = 1. But it's true in practice. And the breakage would be hard to measure if this were ever changed, so a change is unlikely.
impl Packed for u8 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for i8 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for u16 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for i16 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for u32 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for i32 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for u64 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for u128 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for i128 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for i64 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for char {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for f32 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for f64 {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}
impl Packed for usize {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::no()
    } // Doesn't have a fixed size
}
impl Packed for isize {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::no()
    } // Doesn't have a fixed size
}
impl Packed for () {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        IsPacked::yes()
    }
}

impl<T: WithSchema + 'static, const N: usize> WithSchema for [T; N] {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Array(SchemaArray {
            item_type: Box::new(context.possible_recursion::<T>(|context| T::schema(version, context))),
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

impl<T: Packed, const N: usize> Packed for [T; N] {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        T::repr_c_optimization_safe(version)
    }
}
impl<T: Serialize + Packed + 'static, const N: usize> Serialize for [T; N] {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        if N == 0 {
            return Ok(());
        }
        unsafe {
            if T::repr_c_optimization_safe(serializer.file_version).is_false() {
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

impl<T: Deserialize + Packed + 'static, const N: usize> Deserialize for [T; N] {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        if N == 0 {
            return Ok([(); N].map(|_| unreachable!()));
        }

        if unsafe { T::repr_c_optimization_safe(deserializer.file_version) }.is_false() {
            let mut data: [MaybeUninit<T>; N] = unsafe {
                MaybeUninit::uninit().assume_init() //This seems strange, but is correct according to rust docs: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html, see chapter 'Initializing an array element-by-element'
            };
            for idx in 0..N {
                data[idx] = MaybeUninit::new(T::deserialize(deserializer)?); //This leaks on panic, but we shouldn't panic and at least it isn't UB!
            }
            let ptr = &mut data as *mut _ as *mut [T; N];
            let res = unsafe { ptr.read() };
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
                deserializer
                    .reader
                    .read_exact(unsafe { std::mem::transmute::<&mut [MaybeUninit<u8>], &mut [u8]>(slice) })?;
            }
            let ptr = &mut data as *mut _ as *mut [T; N];
            let res = unsafe { ptr.read() };
            Ok(res)
        }
    }
}

impl<T1> Packed for Range<T1> {}
impl<T1: WithSchema> WithSchema for Range<T1> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::new_tuple2::<T1, T1>(version, context)
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

impl<T1: Packed> Packed for (T1,) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        if offset_of_tuple!((T1,), 0) == 0 && std::mem::size_of::<T1>() == std::mem::size_of::<(T1,)>() {
            T1::repr_c_optimization_safe(version)
        } else {
            IsPacked::no()
        }
    }
}
impl<T1: Packed, T2: Packed> Packed for (T1, T2) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        if offset_of_tuple!((T1, T2), 0) == 0
            && std::mem::size_of::<T1>() + std::mem::size_of::<T2>() == std::mem::size_of::<(T1, T2)>()
        {
            T1::repr_c_optimization_safe(version) & T2::repr_c_optimization_safe(version)
        } else {
            IsPacked::no()
        }
    }
}
impl<T1: Packed, T2: Packed, T3: Packed> Packed for (T1, T2, T3) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        if offset_of_tuple!((T1, T2, T3), 0) == 0
            && offset_of_tuple!((T1, T2, T3), 1) == std::mem::size_of::<T1>()
            && std::mem::size_of::<T1>() + std::mem::size_of::<T2>() + std::mem::size_of::<T3>()
                == std::mem::size_of::<(T1, T2, T3)>()
        {
            T1::repr_c_optimization_safe(version)
                & T2::repr_c_optimization_safe(version)
                & T3::repr_c_optimization_safe(version)
        } else {
            IsPacked::no()
        }
    }
}
impl<T1: Packed, T2: Packed, T3: Packed, T4: Packed> Packed for (T1, T2, T3, T4) {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        if offset_of_tuple!((T1, T2, T3, T4), 0) == 0
            && offset_of_tuple!((T1, T2, T3, T4), 1) == std::mem::size_of::<T1>()
            && offset_of_tuple!((T1, T2, T3, T4), 2) == std::mem::size_of::<T1>() + std::mem::size_of::<T2>()
            && std::mem::size_of::<T1>()
                + std::mem::size_of::<T2>()
                + std::mem::size_of::<T3>()
                + std::mem::size_of::<T4>()
                == std::mem::size_of::<(T1, T2, T3, T4)>()
        {
            T1::repr_c_optimization_safe(version)
                & T2::repr_c_optimization_safe(version)
                & T3::repr_c_optimization_safe(version)
                & T4::repr_c_optimization_safe(version)
        } else {
            IsPacked::no()
        }
    }
}

impl<T1: WithSchema, T2: WithSchema, T3: WithSchema> WithSchema for (T1, T2, T3) {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::new_tuple3::<T1, T2, T3>(version, context)
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
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::new_tuple2::<T1, T2>(version, context)
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
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::new_tuple1::<T1>(version, context)
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

#[cfg(feature = "nalgebra")]
impl<T: nalgebra::Scalar> Introspect for nalgebra::Point3<T> {
    fn introspect_value(&self) -> String {
        format!("{:?}", self)
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
}
#[cfg(feature = "nalgebra")]
impl<T: nalgebra::Scalar> Introspect for nalgebra::Vector3<T> {
    fn introspect_value(&self) -> String {
        format!("{:?}", self)
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
}
#[cfg(feature = "nalgebra")]
impl<T: nalgebra::Scalar> Introspect for nalgebra::Isometry3<T> {
    fn introspect_value(&self) -> String {
        format!("{:?}", self)
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Packed + nalgebra::Scalar + Default> Packed for nalgebra::Point3<T> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        let d = nalgebra::Point3::<T>::new(T::default(), T::default(), T::default());
        let p1 = &d.x as *const T;
        let p2 = &d.y as *const T;
        let p3 = &d.z as *const T;

        if std::mem::size_of::<nalgebra::Point3<T>>() == 3 * std::mem::size_of::<T>()
            && p1.offset(1) == p2
            && p1.offset(2) == p3
        {
            IsPacked::yes()
        } else {
            IsPacked::no()
        }
    }
}
#[cfg(feature = "nalgebra")]
impl<T: WithSchema + nalgebra::Scalar> WithSchema for nalgebra::Point3<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Array(SchemaArray {
            item_type: Box::new(T::schema(version, context)),
            count: 3,
        })
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Serialize + Packed + WithSchema + nalgebra::Scalar> Serialize for nalgebra::Point3<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.coords.x.serialize(serializer)?;
        self.coords.y.serialize(serializer)?;
        self.coords.z.serialize(serializer)?;

        Ok(())
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Deserialize + Packed + WithSchema + nalgebra::Scalar + nalgebra::SimdValue + nalgebra::RealField> Deserialize
    for nalgebra::Point3<T>
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(nalgebra::Point3::new(
            <T as Deserialize>::deserialize(deserializer)?,
            <T as Deserialize>::deserialize(deserializer)?,
            <T as Deserialize>::deserialize(deserializer)?,
        ))
    }
}

#[cfg(feature = "nalgebra")]
impl<T: Packed + nalgebra::Scalar + Default> Packed for nalgebra::Vector3<T> {
    unsafe fn repr_c_optimization_safe(_version: u32) -> IsPacked {
        let d = nalgebra::Vector3::<T>::new(T::default(), T::default(), T::default());
        let p1 = &d.x as *const T;
        let p2 = &d.y as *const T;
        let p3 = &d.z as *const T;

        if std::mem::size_of::<nalgebra::Point3<T>>() == 3 * std::mem::size_of::<T>()
            && p1.offset(1) == p2
            && p1.offset(2) == p3
        {
            IsPacked::yes()
        } else {
            IsPacked::no()
        }
    }
}
#[cfg(feature = "nalgebra")]
impl<T: WithSchema + nalgebra::Scalar> WithSchema for nalgebra::Vector3<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Array(SchemaArray {
            item_type: Box::new(T::schema(version, context)),
            count: 3,
        })
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Serialize + Packed + WithSchema + nalgebra::Scalar> Serialize for nalgebra::Vector3<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.x.serialize(serializer)?;
        self.y.serialize(serializer)?;
        self.z.serialize(serializer)?;

        Ok(())
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Deserialize + Packed + WithSchema + nalgebra::Scalar + nalgebra::SimdValue + nalgebra::RealField> Deserialize
    for nalgebra::Vector3<T>
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(nalgebra::Vector3::new(
            <T as Deserialize>::deserialize(deserializer)?,
            <T as Deserialize>::deserialize(deserializer)?,
            <T as Deserialize>::deserialize(deserializer)?,
        ))
    }
}

#[cfg(feature = "nalgebra")]
impl<T: Packed> Packed for nalgebra::Isometry3<T> {}
#[cfg(feature = "nalgebra")]
impl<T: WithSchema> WithSchema for nalgebra::Isometry3<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Array(SchemaArray {
            item_type: Box::new(T::schema(version, context)),
            count: 7,
        })
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Serialize + Packed + WithSchema + nalgebra::Scalar> Serialize for nalgebra::Isometry3<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.translation.vector.x.serialize(serializer)?;
        self.translation.vector.y.serialize(serializer)?;
        self.translation.vector.z.serialize(serializer)?;

        self.rotation.coords.w.serialize(serializer)?;
        self.rotation.coords.x.serialize(serializer)?;
        self.rotation.coords.y.serialize(serializer)?;
        self.rotation.coords.z.serialize(serializer)?;

        Ok(())
    }
}
#[cfg(feature = "nalgebra")]
impl<T: Deserialize + Packed + WithSchema + nalgebra::Scalar + nalgebra::SimdValue + nalgebra::RealField> Deserialize
    for nalgebra::Isometry3<T>
{
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(nalgebra::Isometry3::from_parts(
            nalgebra::Point3::new(
                <T as Deserialize>::deserialize(deserializer)?,
                <T as Deserialize>::deserialize(deserializer)?,
                <T as Deserialize>::deserialize(deserializer)?,
            )
            .into(),
            nalgebra::UnitQuaternion::new_unchecked(nalgebra::Quaternion::new(
                <T as Deserialize>::deserialize(deserializer)?,
                <T as Deserialize>::deserialize(deserializer)?,
                <T as Deserialize>::deserialize(deserializer)?,
                <T as Deserialize>::deserialize(deserializer)?,
            )),
        ))
    }
}

#[cfg(feature = "arrayvec")]
impl<const C: usize> Packed for arrayvec::ArrayString<C> {}

#[cfg(feature = "arrayvec")]
impl<const C: usize> WithSchema for arrayvec::ArrayString<C> {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_string(VecOrStringLayout::Unknown))
    }
}
#[cfg(feature = "arrayvec")]
impl<const C: usize> Serialize for arrayvec::ArrayString<C> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_string(self.as_str())
    }
}
#[cfg(feature = "arrayvec")]
impl<const C: usize> Deserialize for arrayvec::ArrayString<C> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let l = deserializer.read_usize_packed()?;
        if l > C {
            return Err(SavefileError::ArrayvecCapacityError {
                msg: format!("Deserialized data had length {}, but ArrayString capacity is {}", l, C),
            });
        }
        let mut tempbuf = [0u8; C];
        deserializer.read_bytes_to_buf(&mut tempbuf[0..l])?;

        match std::str::from_utf8(&tempbuf[0..l]) {
            Ok(s) => Ok(arrayvec::ArrayString::try_from(s)?),
            Err(_err) => Err(SavefileError::InvalidUtf8 {
                msg: format!("ArrayString<{}> contained invalid UTF8", C),
            }),
        }
    }
}
#[cfg(feature = "arrayvec")]
impl<const C: usize> Introspect for arrayvec::ArrayString<C> {
    fn introspect_value(&self) -> String {
        self.to_string()
    }

    fn introspect_child(&self, _index: usize) -> Option<Box<dyn IntrospectItem>> {
        None
    }
}

#[cfg(feature = "arrayvec")]
impl<V: WithSchema, const C: usize> WithSchema for arrayvec::ArrayVec<V, C> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        Schema::Vector(Box::new(V::schema(version, context)), VecOrStringLayout::Unknown)
    }
}

#[cfg(feature = "arrayvec")]
impl<V: Introspect + 'static, const C: usize> Introspect for arrayvec::ArrayVec<V, C> {
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

#[cfg(feature = "arrayvec")]
impl<V: Packed, const C: usize> Packed for arrayvec::ArrayVec<V, C> {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        V::repr_c_optimization_safe(version)
    }
}

#[cfg(feature = "arrayvec")]
impl<V: Serialize + Packed, const C: usize> Serialize for arrayvec::ArrayVec<V, C> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        unsafe {
            if V::repr_c_optimization_safe(serializer.file_version).is_false() {
                regular_serialize_vec(self, serializer)
            } else {
                let l = self.len();
                serializer.write_usize_packed(l)?;
                serializer.write_buf(std::slice::from_raw_parts(
                    self.as_ptr() as *const u8,
                    std::mem::size_of::<V>() * l,
                ))
            }
        }
    }
}

#[cfg(feature = "arrayvec")]
impl<V: Deserialize + Packed, const C: usize> Deserialize for arrayvec::ArrayVec<V, C> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<arrayvec::ArrayVec<V, C>, SavefileError> {
        let mut ret = arrayvec::ArrayVec::new();
        let l = deserializer.read_usize_packed()?;
        if l > ret.capacity() {
            return Err(SavefileError::ArrayvecCapacityError {
                msg: format!("ArrayVec with capacity {} can't hold {} items", ret.capacity(), l),
            });
        }
        if unsafe { V::repr_c_optimization_safe(deserializer.file_version) }.is_false() {
            for _ in 0..l {
                ret.push(V::deserialize(deserializer)?);
            }
        } else {
            unsafe {
                let bytebuf = std::slice::from_raw_parts_mut(ret.as_mut_ptr() as *mut u8, std::mem::size_of::<V>() * l);
                deserializer.reader.read_exact(bytebuf)?; //We 'leak' Packed objects here on error, but the idea is they are drop-less anyway, so this has no effect
                ret.set_len(l);
            }
        }
        Ok(ret)
    }
}

use std::ops::{Deref, Range};
impl<T: WithSchema + 'static> WithSchema for Box<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        context.possible_recursion::<T>(|context| T::schema(version, context))
    }
}
impl<T> Packed for Box<T> {}
impl<T: Serialize + 'static> Serialize for Box<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.deref().serialize(serializer)
    }
}
impl<T: Deserialize + 'static> Deserialize for Box<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Box::new(T::deserialize(deserializer)?))
    }
}

use std::rc::Rc;

impl<T> Packed for Rc<T> {}
impl<T: WithSchema + 'static> WithSchema for Rc<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        context.possible_recursion::<T>(|context| T::schema(version, context))
    }
}
impl<T: Serialize + 'static> Serialize for Rc<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.deref().serialize(serializer)
    }
}
impl<T: Deserialize + 'static> Deserialize for Rc<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Rc::new(T::deserialize(deserializer)?))
    }
}

impl<T> Packed for Arc<T> {}
impl<T: WithSchema + 'static> WithSchema for Arc<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        context.possible_recursion::<T>(|context| T::schema(version, context))
    }
}
impl<T: Serialize + 'static> Serialize for Arc<T> {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        self.deref().serialize(serializer)
    }
}
impl<T: Deserialize + 'static> Deserialize for Arc<T> {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(Arc::new(T::deserialize(deserializer)?))
    }
}
use byteorder::{ReadBytesExt, WriteBytesExt};
#[cfg(feature = "bzip2")]
use bzip2::Compression;
use memoffset::offset_of_tuple;
use std::any::Any;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
#[allow(unused_imports)]
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::slice;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

impl<T> Packed for RefCell<T> {}
impl<T: WithSchema> WithSchema for RefCell<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        T::schema(version, context)
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

impl<T: Packed> Packed for Cell<T> {
    unsafe fn repr_c_optimization_safe(version: u32) -> IsPacked {
        T::repr_c_optimization_safe(version)
    }
}
impl<T: WithSchema> WithSchema for Cell<T> {
    fn schema(version: u32, context: &mut WithSchemaContext) -> Schema {
        T::schema(version, context)
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
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
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
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_bool)
    }
}
impl WithSchema for AtomicU8 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u8)
    }
}
impl WithSchema for AtomicI8 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i8)
    }
}
impl WithSchema for AtomicU16 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u16)
    }
}
impl WithSchema for AtomicI16 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i16)
    }
}
impl WithSchema for AtomicU32 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u32)
    }
}
impl WithSchema for AtomicI32 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i32)
    }
}
impl WithSchema for AtomicU64 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u64)
    }
}
impl WithSchema for AtomicI64 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i64)
    }
}
impl WithSchema for AtomicUsize {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        match std::mem::size_of::<usize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_u32),
            8 => Schema::Primitive(SchemaPrimitive::schema_u64),
            _ => panic!("Size of usize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}
impl WithSchema for AtomicIsize {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        match std::mem::size_of::<isize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_i32),
            8 => Schema::Primitive(SchemaPrimitive::schema_i64),
            _ => panic!("Size of isize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}

impl WithSchema for bool {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_bool)
    }
}
impl WithSchema for u8 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u8)
    }
}
impl WithSchema for i8 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i8)
    }
}
impl WithSchema for u16 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u16)
    }
}
impl WithSchema for i16 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i16)
    }
}
impl WithSchema for u32 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u32)
    }
}
impl WithSchema for i32 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i32)
    }
}
impl WithSchema for u64 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u64)
    }
}
impl WithSchema for u128 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_u128)
    }
}
impl WithSchema for i128 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i128)
    }
}
impl WithSchema for i64 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_i64)
    }
}
impl WithSchema for char {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_char)
    }
}
impl WithSchema for usize {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        match std::mem::size_of::<usize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_u32),
            8 => Schema::Primitive(SchemaPrimitive::schema_u64),
            _ => panic!("Size of usize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}
impl WithSchema for isize {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        match std::mem::size_of::<isize>() {
            4 => Schema::Primitive(SchemaPrimitive::schema_i32),
            8 => Schema::Primitive(SchemaPrimitive::schema_i64),
            _ => panic!("Size of isize was neither 32 bit nor 64 bit. This is not supported by the savefile crate."),
        }
    }
}
impl WithSchema for f32 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_f32)
    }
}
impl WithSchema for f64 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
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
        serializer.write_u16_packed(*self)
    }
}
impl Deserialize for u16 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u16_packed()
    }
}
impl Serialize for i16 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i16_packed(*self)
    }
}
impl Deserialize for i16 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i16_packed()
    }
}

impl Serialize for u32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u32_packed(*self)
    }
}
impl Deserialize for u32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u32_packed()
    }
}
impl Serialize for i32 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i32_packed(*self)
    }
}
impl Deserialize for i32 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i32_packed()
    }
}

impl Serialize for u64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u64_packed(*self)
    }
}
impl Deserialize for u64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_u64_packed()
    }
}
impl Serialize for i64 {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_i64_packed(*self)
    }
}
impl Serialize for char {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u32_packed((*self).into())
    }
}
impl Deserialize for i64 {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_i64_packed()
    }
}
impl Deserialize for char {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let uc = deserializer.read_u32()?;
        match uc.try_into() {
            Ok(x) => Ok(x),
            Err(_) => Err(SavefileError::InvalidChar),
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
        serializer.write_usize_packed(*self)
    }
}
impl Deserialize for usize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_usize_packed()
    }
}
impl Serialize for isize {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_isize_packed(*self)
    }
}
impl Deserialize for isize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        deserializer.read_isize_packed()
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
        serializer.write_usize_packed(self.load(Ordering::SeqCst))
    }
}
impl Deserialize for AtomicUsize {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        Ok(AtomicUsize::new(deserializer.read_usize_packed()?))
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

impl Packed for AtomicBool {}
impl Packed for AtomicI8 {}
impl Packed for AtomicU8 {}
impl Packed for AtomicI16 {}
impl Packed for AtomicU16 {}
impl Packed for AtomicI32 {}
impl Packed for AtomicU32 {}
impl Packed for AtomicI64 {}
impl Packed for AtomicU64 {}
impl Packed for AtomicIsize {}
impl Packed for AtomicUsize {}

/// A zero-sized marker for troubleshooting purposes.
///
/// It serializes to a magic value,
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
impl Packed for Canary1 {}
impl WithSchema for Canary1 {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Primitive(SchemaPrimitive::schema_canary1)
    }
}

impl WithSchema for Duration {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "Duration".to_string(),
            size: None,
            alignment: None,
            fields: vec![Field {
                name: "Duration".to_string(),
                value: Box::new(Schema::Primitive(SchemaPrimitive::schema_u128)),
                offset: None,
            }],
        })
    }
}
impl Packed for Duration {}
impl Serialize for Duration {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_u128(self.as_nanos())
    }
}
impl Deserialize for Duration {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let temp = deserializer.read_u128()?;
        Ok(Duration::from_secs((temp / 1_000_000_000) as u64) + Duration::from_nanos((temp % 1_000_000_000) as u64))
    }
}

impl Introspect for Duration {
    fn introspect_value(&self) -> String {
        format!("{:?}", self)
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }

    fn introspect_len(&self) -> usize {
        0
    }
}
impl Introspect for SystemTime {
    fn introspect_value(&self) -> String {
        format!("{:?}", self)
    }

    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }

    fn introspect_len(&self) -> usize {
        0
    }
}
impl WithSchema for SystemTime {
    fn schema(_version: u32, _context: &mut WithSchemaContext) -> Schema {
        Schema::Struct(SchemaStruct {
            dbg_name: "SystemTime".to_string(),
            size: None,
            alignment: None,
            fields: vec![Field {
                name: "SystemTimeDuration".to_string(),
                value: Box::new(Schema::Primitive(SchemaPrimitive::schema_u128)),
                offset: None,
            }],
        })
    }
}
impl Packed for SystemTime {}
impl Serialize for SystemTime {
    fn serialize(&self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        match self.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(nanos) => {
                let temp = nanos.as_nanos();
                if temp >= 1u128 << 120 {
                    return Err(SavefileError::GeneralError {
                        msg: "Savefile cannot handle dates where the year is larger than ca 10^19 years.".to_string(),
                    });
                }
                serializer.write_u128(temp)?;
            }
            Err(err) => {
                //Before UNIX Epoch
                let mut temp = err.duration().as_nanos();
                if temp >= 1u128 << 120 {
                    return Err(SavefileError::GeneralError {
                        msg: "Savefile cannot handle dates much earlier than the creation of the universe.".to_string(),
                    });
                }
                temp |= 1u128 << 127;
                serializer.write_u128(temp)?;
            }
        }
        Ok(())
    }
}

impl Introspect for std::time::Instant {
    fn introspect_value(&self) -> String {
        format!("{:?}", self)
    }
    fn introspect_child<'a>(&'a self, _index: usize) -> Option<Box<dyn IntrospectItem<'a> + 'a>> {
        None
    }
}

fn u128_duration_nanos(nanos: u128) -> Duration {
    if nanos > u64::MAX as u128 {
        Duration::from_nanos((nanos % 1_000_000_000) as u64) + Duration::from_secs((nanos / 1_000_000_000) as u64)
    } else {
        Duration::from_nanos(nanos as u64)
    }
}
impl Deserialize for SystemTime {
    fn deserialize(deserializer: &mut Deserializer<impl Read>) -> Result<Self, SavefileError> {
        let mut temp = deserializer.read_u128()?;
        if temp >= (1u128 << 127) {
            temp &= (1u128 << 127) - 1; //Before UNIX Epoch
            return Ok(SystemTime::UNIX_EPOCH - u128_duration_nanos(temp));
        } else {
            return Ok(SystemTime::UNIX_EPOCH + u128_duration_nanos(temp));
        }
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
                } else if item.has_children {
                    ">"
                } else {
                    " "
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
impl Introspector {
    /// Returns a new Introspector with no limit to the number of fields introspected per level
    pub fn new() -> Introspector {
        Introspector {
            path: vec![],
            child_load_count: usize::MAX,
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

    fn dive(
        &mut self,
        depth: usize,
        object: &dyn Introspect,
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
    pub fn do_introspect(
        &mut self,
        object: &dyn Introspect,
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
            frames,
            cached_total_len: total,
        };
        Ok(accum)
    }
}
