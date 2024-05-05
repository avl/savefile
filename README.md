![build](https://github.com/avl/savefile/actions/workflows/rust.yml/badge.svg)

**Having trouble with new version 0.17? - See upgrade guide further down in this document!**

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
savefile = "0.17"
savefile-derive = "0.17"
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

See the docs for more information, including schema-versioning: https://docs.rs/savefile/latest/savefile/ .

# Changelog

## 0.17.1

Just minor improvements to documentation.

## 0.17.0

This is a big change! With 0.17 Savefile gains yet another major function: Support for
making dynamically loaded plugins. I.e, a mechanism to allow rust code to be divided
up into different shared libraries (.so on linux, .dll on windows), and allow calls
across library boundaries.

Using this feature requires using the crate 'savefile-abi'. Regular use of savefile
can continue as usual.

The data format for schemas has changed, but in a backward compatible way. I.e, savefile
0.17 can still read data saved by 0.16 and earlier. However, 0.16 can't read data
saved by 0.17.

Another thing in 0.17.0: We're starting to use 'release-plz' to manage releases.
Hopefully this will make the releases more professional, with correct git tags, git releases etc.

Also, 0.17 can now sometimes dramtically optimize reading sequences of enums. This will work
for enums that have #[repr(u8)] or similar, provided the type contains no padding.

Yet another upgrade is that Savefile now supports enums with more than 256 variants!

### Upgrade guide from 0.16 -> 0.17

Unfortunately 0.17 is quite a big release. Some changes will be required to upgrade.

Here is a short guide:

1: Schemas have been expanded. 

1.1: Schema::Vector takes a 2nd parameter. Just set it to 'VecOrStringLayout::Unknown' or `VecOrStringLayout::default()`.

1.2: Field of Schema::Struct now takes an 'offset' parameter. It is safe to always set to None. Some
parameters have become private, so now you need to use a 'new'-function to create instances of Schema::Struct.
The reason for this is that it is needed to guarantee soundness since some of the new fields must be
given correct values to avoid unsound behaviour. There is an unsafe function to initialize these,
so they are not completely hidden.

1.3: The field 'discriminator' of SchemaEnum Variant has been renamed to 'discriminant' (since this is
what the rust project calls it)

1.4: The SchemaEnum type has gained the field 'discriminant_size'. This is the number of bytes needed to
encode the discriminant. Set to 1 for enums which will never have more than 256 fields. Set to 2 for bigger
enums. If you ever need an enum to have more than 65536 fields, set it to 4. Note that the
SchemaEnum type also now has private fields, and also needs to be constructed using 'new'.

1.5: The WithSchema::schema function now takes a context object. You can just pass this through for
most data types. Only smart pointers, containers, Box etc need ot use this, to guard against 
recursion. See documentation of WithSchemaContext.

1.6: The 'ReprC'-trait has been renamed to 'Packed'. It is identical in every other way.
Upgrading is as simple as a search-and-replace.


1.7: Several savefile trait implementations have now gained 'static-bounds. For example,
Box<T>, Vec<T> and many more now require T:'static. There was no such bound before, but
since references cannot be deserialized, it was still typically not possible to deserialize
anything containing a reference. 

It turns out there is a usecase for serializing objects with lifetimes: Things like
Cow<str> can be useful. Everything the deserializer produces must still have 'static lifetime in
practice, because of how the Deserialize trait is defined (there's no other lifetime the
return value can have).

Serializing things with lifetimes is still possible, the only place where 'static is required
is the contents of containers such as Box, Vec etc. The reason is that the new recursion
support needs to be able to create TypeIds, and this is only possible for objects with
'static lifetime.


## 0.16.5

Minor change to savefile-derive to avoid triggering warnings when Rust RFC 3373 lands. See
bug #36 https://github.com/avl/savefile/issues/36 for more information.

## 0.16.4

Support for boxed slices. I.e, Savefile can now serialize data of type ```Box<[T]>```.

## 0.16.3

Just some fixes to the test suite, needed to make github actions CI work. 

## 0.16.2

Fixes multiple problems. The most impactful being that 0.16.1 could only be built
using a nightly rust compiler.

It also provides an optional feature to integrate 'savefile-derive' into 'savefile'.

Just activate the feature 'derive', and you can then use savefile without an explicit
dependency on 'savefile-derive'. Just do

```rust
use savefile::prelude::*;

#[derive(Savefile)]
struct MyStruct { 
    //...    
}
```
And you're good to go!


## 0.16.1

Fix a minor issue where the ```#[savefile_introspect_ignore]``` was not accepted
in combination with ```#[derive(SavefileIntrospectOnly)]```.


## 0.16
Major performance improvements, slight API adjustments.

The Serializer and Deserializer types now are parameterized on the type of Write-implementation
they take. Previously, a ```&mut dyn Write``` type was used, but it turns out this performs
very much worse than having the type of the writer be generic.

This only affects manual implementations of the Serialize and Deserialize traits, usage
of the derive-macro is not affected. To fix any manual implementation of Serialize
and Deserialize, all you have to do is to change ```Serializer``` into ```Serializer<impl Write>``` 
and ```Deserializer``` into ```Deserializer<impl Write>```.

Also, dependencies have been updated:
```
smallvec 1.0 -> 1.11
indexmap 1.6 -> 1.9
byteorder 1.2 -> 1.4
```

If you are serializing smallvec and/or indexmap, you need to step to version 1.11 and 1.9
respectively. No change in functionality is expected.

This version also includes big changes to the derive macro. It now tries to detect when it's
safe to write multiple fields, or entire structs, at once. By inspecting the layout of
the type, it can detect when several fields are placed adjacent to each other in memory,
and then just write their bytes all at once.

This release is the largest change made to savefile in a long while. The test suite has
been run, but still, there may be bugs.


## 0.15

Automatic activation of #[savefile_unsafe_and_fast]-optimization for types which can
safely support it.

It had always been a bit sad that Savefile didn't perform as fast as it could have
on stable rust. With 0.14, this restriction is mostly lifted.

However, getting the speed boost still required unsafe code. 

With 0.15, unsafe code is no longer required to get a speed boost. In fact,
in many situations, nothing special at all needs to be done.

Savefile now contains code to automatically check if a struct has a packed
representation, and if it does, it automatically behaves just the same as with
the manual opt-in we hade before. The difference is that the derived serializers
now automatically determine if the type actually fulfills the requirements needed.

Note! It might in some situations be necessary to use `#[repr(C)]` to get the speedup.


## 0.14.3 Bugfixes to 0.14 release

The 0.14 release contained some bugs. It was actually impossible to serialize
collections containing many standard types. This is fixed in 0.14.3.

## 0.14 Major changes to Packed (previously 'ReprC')-system

One of the strong points of Savefile is the support for very quickly serializing
and deserializing vectors of simple copy-datatypes.

The previous way to do this was to define a struct like this:

```rust
#[derive(Savefile,ReprC)]
#[repr(C)]
pub struct Example {
    pub x: u32,
    pub y: u32,
}
```

The new way is:

```rust
#[derive(Savefile)]
#[savefile_require_fast]
#[repr(C)]
pub struct Example {
    pub x: u32,
    pub y: u32,
}
```

The old solution implemented a marker trait 'Packed' for the type. Then, Savefile relied on
specialization to be able to serialize vectors and arrays of these types much faster
by simply copying large regions of bytes.

At the time, it seemed like the fact that specialization was only available on nightly would
be a temporary nuisance. Today, 2023, it is starting to seem like specialization might _never_
land in stable rust.

Because of this, we are now bringing the speedups to stable, by abandoning specialization!

We are now implementing 'Packed' (previously known as 'ReprC') for _all_ types, and then 
just returning false for types which don't support the optimization. 

This has some drawbacks. Previously, ReprC (early name of 'Packed') was an unsafe trait. 
It wasn't mandatory, but if you knew what you were doing you could get extra performance, 
under responsibility.

But now, Packed is to be implemented by all types. And we don't want it to be necessary to use
unsafe code to be able to use Savefile. However, to the user of Savefile, there isn't much
difference, except the Packed trait always being derived, and another way being used to opt-in
to the unsafe but performant optimization.

I don't know of a way to require 'unsafe' keyword to a derive macro, so we use a deliberately
eye-catching non-conforming name ```#[savefile_unsafe_and_fast]``` to signal danger.

Update: With 0.15, savefile_unsafe_and_fast is mostly not needed. Instead there's a
new attribute, 'savefile_require_fast'. Using it, means we get a compile error if fast operation 
was not possible because of mis-alignment or padding. Without 'savefile_require_fast'-attribute
savefile works even if alignment is bad, it just goes slower.

## 0.13 Support generic structs without Savefile type-constraints

Previously, if you tried to implement the following struct:

```rust
#[derive(Savefile)]
pub struct ExampleGeneric<T> {
    pub x: T
}
```

You would get compiler errors, because T cannot be serialized using Savefile.
However, as a user, it may be that the struct ExampleGeneric doesn't always need
to be serializable. What is really required, is that whenever an attempt is made to
serialize a particular instance, then the type of that instance has to be serializable.

Other derive-macros, like the 'Debug' macro, don't work like this. They
only implement Debug for such a struct if all type parameters themselves implement Debug,
avoiding the compiler error.

Beginning with 0.13, savefile works the same way.

In the future, it could be possible to make the type requirements even more clever,
only requiring that types which are actually used during serialization support the
savefile-traits. 

Versions 0.14 and 0.14.1 contained minor errors in the Readme. This is fixed in 0.14.2, 
it is otherwise the same.


## 0.12 Support for char

It turns out that the very basic type 'char' was not actually supported by savefile.

This release fixes this oversight.

## 0.11.1 Support for unit structs

Savefile-derive gains support for unit structs. I.e, the following now compiles:

```rust
#[derive(Savefile)]
struct MyUnitStruct;
```



## 0.11 Improve performance of bit-vec serialization, upgrade arrayvec, support more datatypes

Savefile lacked support for u128 and i128 types. This is fixed.

Also: Slight improvements to the serialization and deserialization of bit-vec.
The on-disk format has changed, but the deserializer knows how to read the old versions.
Bit-vec are no longer supported on big-endian machines. Please get in touch if
this is a big limitation. The reason for this change is that it allows to use a more efficient
format. On a big-endian machine conversion would be needed, and this would be expensive and hard to test
without access to a big-endian machine.

PRs for big-endian support would be accepted.

Also, adds support for bit-set crate and rustc-hash.

Arrayvec upgraded to version 0.7. They have changed their API slightly. See arrayvec docs.

This version updates the 'syn' and 'quote' dependencies to versions 1.0. 

Savefile now supports HashSet and HashMap with custom hashers. Deserialize is only supported
if the hasher implement Default (there's no way to provide a state-ful hasher to deserialized hashmaps).


## 0.10.1 Make dependencies even more configurable, and upgrade some dependencies

The following configurable features have been created:

 * "compression" - Enables compression and decompression (using library 'bzip2')
 * "encryption" - Enables encryption support (using library 'ring')

The following dependencies have been made configurable: 
"bit-vec", "arrayvec", "smallvec", "indexmap", "parking_lot" 
 
Also, the following dependencies have been upgraded:
 * parking_lot, from 0.11 to 0.12
 * rand, from 0.7 to 0.8 (only used by 'encryption' feature)
 * bzip2, from 0.3.2 to 0.4 (only used by 'compression' feature)

## 0.9.1 Reduce default dependencies, and some other improvements

### More ergonomic load_file-method


The load_file method's path parameter has been changed to accept anything 
implementing AsRef<Path>.  Previously, a &str was required, which meant that 
the idiomatic Path and PathBuf types were not accepted.

*Migration note*: If you were specifying type parameters explicitly to load_file, or similar
functions, you now need to add a ",_". So

```rust

    let object = load_file::<MyType>("save.bin",0);
```
Must become:

```rust

    let object = load_file::<MyType,_>("save.bin",0);
```

However, an easier option which has always worked and continues to work is:

```rust
    let object : MyType = load_file("save.bin",0);
```


I hope this does not cause too many problems. The reason is that having functions which open files
require '&amp;str' was never a good design, since in principle there could be files whose names are not
actually valid utf8. Such files would not be possible to open using Savefile with the old design.

### Make bzip2 and ring dependencies optional

Put bzip2 and ring dependencies behind feature flags. This makes it easy
for users who do not wish to use compression or encryption to opt out of
these dependencies.

These features are not active by default, so be sure to enable them in 
Cargo.toml like this, if you want to use them:

```
savefile = { version = "0.14", features = ["ring","bzip2"] }
```

Arguably, savefile should never have included this support, since it is something
that can really be added easily by other crates. There is some convenience
having it built-in though, hopefully making it configurable provides the best
of both worlds.

### Add SavefileNoIntrospect-derive

It's now possible to opt out of automatically deriving the Introspect-trait,
but still automatically derive the serialization traits. It was previously
possible to do the opposite, automatically deriving the introspect trait but
not the serialization traits. This gap is now filled.

To derive all traits: ```#[derive(Savefile)]```

To derive only Introspect: ```#[derive(SavefileIntrospectOnly)]```

To derive all but Introspect: ```#[derive(SavefileNoIntrospect)]```




## 0.8.3 Fix bug with savefile_introspect_ignore attribute

Specifying the savefile_introspect_ignore on a field triggered a bug if
that field was not the last field of the datatype. The bug caused a mismatch
in the index of the field during introspect, which would make fields
not visible through introspection.

## 0.8.2 Update dependencies

* parking_lot from 0.10 -> 0.11
* smallvec 1.4 -> 1.6 (made possible by parking_lot upgrade)
* Make Removed<T> implement Clone, Copy, PartialEq etc. Removed fields shouldn't limit what traits a struct can implement.
* Also make Removed<T> implement Send and Sync. It's a zero-sized type, there are no issues with threading here.
* Add support for std::borrow::Cow. Thanks to github user PonasKovas for this patch!


## 0.8.1 Stop depending on the 'failure' crate

This also means that SavefileError now (finally) implements the Error trait.

## 0.8.0 Support for min_const_generics

Savefile now supports serializing and deserializing arbitrarily sized arrays, even
on stable rust.

Note that 0.8.0 is compatible with Rust 1.51 and later only. If you need support
for older rust versions, you should stick with 0.7.x.

## 0.7.5 Deduplicaton of Arc<str>

Previously, serialization of Arc<str> was not possible. Support is now added,
including deduplication of str objects. Note that the deduplication is
not actually in the serialized format, just in the result in memory after
deduplication. The deduplication does not know how the memory graph looked before
saving, it simply makes it so that identical strings are backed by the same 
memory after loading.

This could cause problems if the code is somehow dependent on the addresses
of &str objects. However, this should be rather rare in practice.
Just file a bug if you feel that this could be a problem!


## 0.7.4 Add introspect for PathBuf

PathBuf did not implement Introspect, which had the effect that trying to use derive(Savefile) on
anything containing a PathBuf would fail, since the derive macro requires all components to implement all
the Savefile traits.

## 0.7.3 Support for co-existence with Serde

Savefile-derive would previously not work correctly if one tried to use it in a crate where Serde was also used.

This has been fixed.

Also, support for serializing/deserializing PathBuf.

## 0.8.0

(Not published as of yet)

* Support for arbitrary size arrays, even on stable (thanks to
min_const_generics now supported in rust).

Minimum supported rust version for 0.8.x is 1.51.

## 0.7.2 Support for stable compilers

Savefile is now usable with a stable compiler, not just nightly.

When run on stable, the following features stop working:

NOTE! The below is old information, no longer at all valid from 0.16 and onward.

* The whole 'ReprC' subsystem. This means serialization of byte arrays 
(or other small copy-types) is not as fast as it could be. The slow-down
can be several orders of magnitude.

* Serialization of arbitrary sized arrays. On stable, only arrays of sizes
0-4 are supported.

* Specialisation of introspection for hashmaps with string keys. This
means introspection for hashmaps is not as nice.

The nightly-only features are activated automatically when a nightly compiler
is used.

Also, support for Arc<[T]> .


## 0.7.1 Better support for ArrayVec

The arrayvec crate has long been a dependency, but the actual ArrayVec type was
not supported. We did support ArrayString, but not until 0.7.1 was ArrayVec itself supported.

## 0.7.0 Update some stale dependencies

The dependencies on bitvec, arrayvec and parking_lot where to old versions. They have been updated to:

bit-vec = 0.6

arrayvec = 0.5

parking_lot = 0.10




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

The savefile docs are available at: https://docs.rs/savefile/latest/savefile/

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

# Troubleshooting

Common errors:

## "the trait bound `MyStuff: WithSchema` is not satisfied"
This probably means you've forgotten to derive the Savefile-traits. Add a `#[derive(Savefile)]`.

## the trait `ReprC` is not implemented

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
Copyright 2018 Anders Musikka

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

```
