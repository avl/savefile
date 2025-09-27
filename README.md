![build](https://github.com/avl/savefile/actions/workflows/rust.yml/badge.svg)

**Having trouble with new version 0.20? - See upgrade guide further down in this document!**

# Introduction to Savefile 

Savefile is a crate to effortlessly serialize rust structs and enums. It uses
an efficient binary format. It can serialize to anything implementing the 
Write trait, and then deserialize from anything implementing the Read trait. This 
means that savefile can be used to easily save in-memory data structures to 
disk for persistent storage.

Docs: https://docs.rs/savefile/latest/savefile/

# Capabilities

 * **Easy to use** Most std datatypes are supported, and the derive macro can be used for most user-types.
 * **Reliable** - Savefile has an extensive test suite. 
 * **Backward compatible** - Savefile supports schema-versioning, with built-in verification and detailed error messages on schema mismatch.
 * **Fast** - Savefile can serialize/deserialize many data types quickly by *safely* treating them as raw bytes.
 * **Safe** - Savefile can be used without requiring any unsafe code from the user.


# Savefile-Abi

Savefile-Abi is a related crate, which allows publishing forward- and backward compatible
shared libraries, written in rust, to be used as binary plugins in rust-programs.

Docs: https://docs.rs/savefile-abi/latest/


# Usage
Cargo.toml:
```toml
savefile = "0.20"
savefile-derive = "0.20"
```

main.rs:
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

# Docs

The savefile docs are available at: https://docs.rs/savefile/latest/savefile/

# Changelog

See the [changelog](https://github.com/avl/savefile/blob/master/CHANGELOG.md).

# Features and goals

Features savefile has:

 * Fast binary serialization and deserialization
 * Support for old versions of the save format
 * Completely automatic implementation using "custom derive". You do not have to
 figure out how your data is to be saved.

Features savefile does not have:
 * Support for recursive data-structures
 
Features savefile does not have, and will not have:

 * Support for external protocols/data formats. There'll never be json, yaml,
 xml or any other backends. Savefile uses the savefile format, period.
 * Support for serializing graphs. Savefile can serialize your data if it has a
 tree structure in RAM, _without_ loops. 
 * Support for serializing boxed traits ("objects"). You can (probably) hack this in by manually
 implementing the Serialize and Deserialize traits and somehow select concrete types in
 the deserializer manually.

# Upgrade Guide

## Upgrading from pre 0.16.x:

### "the trait bound `MyStuff: WithSchema` is not satisfied"
This probably means you've forgotten to derive the Savefile-traits. Add a `#[derive(Savefile)]`.

### the trait `ReprC` is not implemented

This one is easy. `ReprC` has been renamed to `Packed`. Just change to `Packed` and things should work. 

# License

Savefile is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

MIT License text:

```
Copyright 2018-2025 Anders Musikka

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

```
