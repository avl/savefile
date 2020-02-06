[![Build Status](https://travis-ci.org/avl/savefile.svg?branch=master)](https://travis-ci.org/avl/savefile)

# Introduction to Savefile 

Savefile is a library to effortlessly serialize rust structs and enums. It uses
an efficient binary format. It can serialize to anything implementing the 
Write trait, and then deserialize from anything implementing the Read trait. This 
means that savefile can be used to easily save in-memory data structures to 
disk for persistent storage.

You may ask what savefile brings to the table that serde doesn't already do
better. The answer is: Not that much! Savefile is less capable, and not as well tested.
It does have versioning support built-in as a first class feature.

Savefile is written by its author to solve exactly the problem the author has to solve.
It is provided here as open source in the hope that it may prove useful to others, but
there are no guarantees and there may be bugs!


Cargo.toml:
````
savefile="0.5.0"
savefile-derive="0.5.0"
````

# Sample 

```rust
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


# Changelog

## 0.6.1 Fix bad reference to SavefileError

If you get compilation error saying that SavefileError is not declared, you need this small fix.

## 0.6.0 Critical fix to encryption-routines

This is a breaking change. Hopefully the last one! There is now a more detailed file header, which may
make it possible to avoid breaking changes to the base binary framework in the future.

There were two bugs in how encrypted files were encrypted and decrypted. Data corruption could occur.

Also, savefile now supports bzip2-compression. Like the encryption-support, this is just for convenience. It should probably,
earnestly, belong out of tree. But I kind of like the idea of "batteries included".




## 0.5.0 Introspection

Savefile now includes an introspection feature. See more in the docs.


## 0.4.0 Breaking Change

I just realized that 'ignore' was a very bad name for a custom attribute, since this
is now a built-in attribute into the Rust language. 

I have prefixed all savefile-attributes with the string "savefile_" .

This breaks existing code, in a rather silent way. The fix is simply to update
all usages of savefile-attributes to include prefix 'savefile_' . The samples in the
docs are correct.


## 0.3.0 Breaking Change

Also, version 0.3.0 breaks binary compatibility with 0.2.*. This is because
arrays of generic length are now supported (using Rust nightly's const_generics-feature).
Previously short arrays were supported, and were (hackishly) serialized as tuples.
Now arrays are truly supported, although very large arrays may cause the stack to be
exhausted since the deserialization framework treats arrays as values. This means
that any arrays serialized using 0.2.* cannot be deserialized using 0.3.*. 

Contact me if this is a showstopper. My general feeling, though, is that there
are no users of this software except the author.


# Docs

The savefile docs are available at: https://docs.rs/savefile/0.6.1/savefile/

# Features and goals

Features savefile has:

 * Fast binary serialization and deserialization
 * Support for old versions of the save format
 * Completely automatic implementation using "custom derive". You do not have to
 figure out how your data is to be saved.
 
Features savefile does not have, and will not have:

 * Support for external protocols/data formats. There'll never be json, yaml,
 xml or any other backends. Savefile uses the savefile format, period.
 * Support for serializing graphs. Savefile can serialize your data if it has a
 tree structure in RAM, _without_ loops.
 * Support for serializing boxed traits ("objects"). You can (probably) hack this in by manually
 implementing the Serialize and Deserialize traits and somehow select concrete types in
 the deserializer manually.


# License

Savefile is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

MIT License text:

```
Copyright 2018 Anders Musikka

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

```
