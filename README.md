[![Build Status](https://travis-ci.org/avl/savefile.svg?branch=master)](https://travis-ci.org/avl/savefile)

# WARNING

THIS IS STILL VERY EXPERIMENTAL! THERE WILL BE BUGS.

This software is very young and possibly NOT ready for use yet :-) .
You have been warned. If you're looking for a high quality 
serialization library for rust, you should probably look at serde instead: 
https://github.com/serde-rs/serde


# Savefile 

Savefile is a library to effortlessly serialize rust structs and enums, in
an efficient binary format, to anything implementing the Write trait, and 
then deserializing the same from anything implementing the Read trait. This 
means that savefile can be used to easily save in-memory data structures to 
disk for persistent storage.

You may ask what savefile brings to the table that serde doesn't already do
better. The answer is: Not much! However, Savefile is much smaller and less 
complex, which could sometimes be an advantage in itself. Savefile also
has features to easily maintain backward compatibility to old versions
of the software.


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

# Docs

The savefile docs are available at: https://docs.rs/crate/savefile/
Make sure you read the docs for the correct version.


# Obligatory completely unfair benchmark

It is a long standing tradition that small minimalistic libraries always have to be benchmarked
against the industry standard, in a way carefully selected to make the new library appears
in the best possible light.

In this case, I've selected the case of a Vec of 1000 elements of the following small plain data struct, using
Savefile and Serde Bincode.

```rust
#[derive(ReprC, Clone, Copy, Debug, Savefile)]
pub struct BenchStruct {
    x: usize,
    y: usize,
    z: u8,
    pad1:u8,
    pad2:u8,
    pad3:u8,
    pad4:u32,
}
```

This is unfair, since Savefile, given the ReprC marker, will handle this case by writing the raw memory directly to file
without any processing. That said, running this gives:

```
test bench_savefile_serialize ... bench:       1,153 ns/iter (+/- 892)
test bench_serde_serialize    ... bench:      27,518 ns/iter (+/- 34)
```

The full source is in the savefile-test crate, in the savefile github repo: https://github.com/avl/savefile



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
