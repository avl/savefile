#![feature(alloc)]
#![feature(allocator_api)]
#![recursion_limit = "256"]
#![feature(test)]
#![feature(specialization)]
#![feature(attr_literals)]
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
    #[versions="0..0"] //Only version 0 had this field
    strength : Removed<u32>,
    inventory : Vec<String>,
    #[versions="1.."] //Only versions 1 and later have this field
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

When adding or removing fields, use the #[versions] attribute.

The syntax is one of the following:

```text
#[versions = "N.."]  //A field added in version N
#[versions = "..N"]  //A field removed in version N+1. That is, it existed up to and including version N.
#[versions = "N..M"] //A field that was added in version N and removed in M+1. That is, a field which existed in versions N .. up to and including M.
```

Removed fields must keep their deserialization type. This is easiest accomplished by substituting their previous type
using the `Removed<T>` type. `Removed<T>` uses zero space in RAM, but deserializes equivalently to T (with the
result of the deserialization thrown away). 

Savefile tries to validate that the `Removed<T>` type is used correctly. This validation is based on string
matching, so it may trigger false positives for other types named Removed. Please avoid using a type with
such a name. If this becomes a problem, please file an issue on github.

Using the #[versions] tag is critically important. If this is messed up, data corruption is likely.

When a field is added, its type must implement the Default trait (unless the default_val or default_fn attributes
are used).

There also exists a default_val, a default_fn and a versions_as field. More about these below:

## The versions attribute

Rules for using the #[versions] attribute:

 * You must keep track of what the current version of your data is. Let's call this version N.
 * You may only save data using version N (supply this number when calling `save`)
 * When data is loaded, you must supply version N as the memory-version number to `load`. Load will
   still adapt the deserialization operation to the version of the serialized data.
 * The version number N is "global". All components of the saved data must have the same version. 
 * Whenever changes to the data are to be made, the global version number N must be increased.
 * You may add a new field to your structs, iff you also give it a #[versions = "N.."] attribute. N must be the new version of your data.
 * You may remove a field from your structs. If previously it had no #[versions] attribute, you must
   add a #[versions = "..N-1"] attribute. If it already had an attribute #[versions = "M.."], you must close
   its version interval using the current version of your data: #[versions = "M..N-1"]. Whenever a field is removed,
   its type must simply be changed to Removed<T> where T is its previous type. You may never completely remove 
   items from your structs. Doing so removes backward-compatibility with that version. This will be detected at load.
   For example, if you remove a field in version 3, you should add a #[versions="..2"] attribute.
 * You may not change the type of a field in your structs, except when using the versions_as-macro.


 
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
    #[default_val="42"]
    #[versions="1.."]
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
    #[default_fn="make_hello_pair"]
    #[versions="1.."]
    new_field: (String,String)
}
# fn main() {}

```

## The versions_as attribute

The versions_as attribute can be used to support changing the type of a field.

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
    #[versions_as="0..0:u8"]
    #[versions="1.."]
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

This is dangerous. You, as implementor of the `ReprR` trait take full responsibility
that all the following rules are upheld:

 * The type T is Copy
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
    #[versions="0..0"] //Only version 0 had this field
    strength : Removed<u32>,
    inventory : Vec<String>,
    #[versions="1.."] //Only versions 1 and later have this field
    skills : Vec<String>,
    #[versions="2.."] //Only versions 2 and later have this field
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
mod savefile;
