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
serialize and deserialize arbitrary rust struct and enums into
an efficient and compact binary version controlled format.

The design use case is any application that needs to save large
amounts of data to disk, and support loading files from previous
versions of the program (but not from later versions!).


# Example

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
you decide that your game mechanics doesn't really need to track the strength of the player. But
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

For Savefile to be able to load an save a type T, that type must implement traits 
`WithSchema`, `Serialize` and `Deserialize` . The custom derive macro Savefile derives
all of these.

You can also implement these traits manually. Manual implementation can be good for:

1: Complex types for which the Savefile custom derive function does not work. For
example, trait objects or objects containing pointers.
2: Objects for which not all fields should be serialized, or which need complex
initialization (like running arbitrary code during deserialization).

## WithSchema

The `WithSchema` trait represents a type which knows which data layout it will have
when saved. 

## Serialize

The `Serialize`trait represents a type which knows how to write instances of itself to
a `Serializer`.

## Deserialize

The `Deserialize`trait represents a type which knows how to read instances of itself from a `Deserializer`.


# Rules for managing versions

The key to supporting multiple versions of your data-structures, is using the #[versions] attribute

The syntax is one of the following:

```text
#[versions = "N.."]  //A field added in version N
#[versions = "..N"]  //A field removed in version N+1. That is, it existed up to and including version N.
#[versions = "N..M"] //A field that was added in version N and removed in M+1. That is, a field which existed in versions N .. up to and including M.
```

Removed fields must keep their deserialization type. This is easiest accomplished by substituting their previous type
using the `Removed<T>` type. `Removed<T>` uses zero space in RAM, but deserializes equivalently to T. 

Savefile tries to validate that the `Removed<T>` type is used correctly. This validation is based on string
matching, so it may trigger false positives for other types named Removed. Please avoid using a type with
such a name. If this becomes a problem, please file an issue on github.

Using the #[versions] tag is critically important. If this is messed up, data corruption is likely.

## Rules

 * You must keep track of what the current version of your data is. Let's call this version N.
 * You may only save data using version N (supply this number when calling `save`)
 * When data is loaded, you must supply version N as the memory-version number to `load`. Load will
   still adapt the deserialization operation to the version of the serialized data.
 * The version number N is "global". All components of the saved data must have the same version. 
 * Whenever changes to the data are to be made, the global version number must be increased.
 * You may add a new field to your structs, iff you also give it a #[versions = "N.."] attribute. N must be the new version of your data.
 * You may remove a field from your structs. If previously it had no #[versions] attribute, you must
   add a #[versions = "..N"] attribute. If it already had an attribute #[versions = "M.."], you must close
   its version interval using the current version of your data: #[versions = "M..N"]. Whenever a field is removed,
   its type must simply be changed to Removed<T> where T is its previous type. You may never completely remove 
   items from your structs. Doing so removes backward-compatibility with that version. This will be detected at load.
 * You may not change the type of a field in your structs.



# Speeding things up

Now, let's say we want to add a list of all positions that our player have visited,
so that we can provide a instant-replay function to our game. The list can become
really long, so we want to make sure that the overhead when serializing this is
as low as possible.

Savefile has an unsafe trait `ReprC` that you can implement for a type T. This instructs
Savefile to optimize serialization of Vec<T> into being just a very fast memory copy.

This is dangerous. You, as implementor of the `ReprR` trait take full responsibility
that all the following rules are upheld:

 * The type T is Copy
 * The type T is a struct. Using it on enums will probably lead to silent data corruption.
 * The type is represented in memory in an ordered, packed representation. Savefile is not
 clever enough to inspect the actual memory layout and adapt to this, so the memory representation
 has to be all the types of the struct fields in a consecutive sequence without any gaps. Note
 that the #[repr(C)] trait does not do this - it will include padding if needed for alignment
 reasons. You should not use #[repr(packed)], since that may lead to unaligned struct fields.
 If you really want the performance boost of the Savefile `ReprC`-trait, you should use #[repr(C)]
 combined with manual padding, if necessary.
 * The type is either a struct, or an enum without fields.
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
detected in release builds. Serializing or deserializing each `ReprC` struct at least once somewhere in your test suite
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
