# Welcome to Savefile-abi!

Full docs: https://docs.rs/savefile/latest/savefile-abi/

Savefile-abi is a crate that is primarily meant to help building binary plugins using Rust.

Note! This is a work-in-progress.

Remaining todo:

* Support tuples as parameters to trait methods!

```toml
savefile-abi = "0.17.0-beta.1"
savefile = "0.17.0-beta.1"
savefile-derive = "0.17.0-beta.1"
```

# Example

Let's say we have a crate that defines this trait for adding u32s:

*InterfaceCrate*
```rust
use std::fmt::{Debug, Formatter};
use savefile_derive::Savefile;
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version=0)]
pub trait AdderInterface {
    fn add(&self, x: u32, y: u32) -> u32;
}

```

Now, we want to implement addition in a different crate, compile it to a shared library
(.dll or .so), and use it in the first crate (or some other crate):

*ImplementationCrate*
```rust
use IntefaceCrate::{AdderInterface};
use savefile_derive::savefile_abi_export;

#[derive(Default)]
struct MyAdder { }

impl AdderInterface for MyAdder {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

/// Export this implementation as the default-implementation for
/// the interface 'AdderInterface', for the current library.
savefile_abi_export!(MyAdder, AdderInterface);

```

We add the following to Cargo.toml in our implementation crate:

```toml
[lib]
crate-type = ["cdylib"]
```

Now, in our application, we add a dependency to *InterfaceCrate*, but not
to *ImplementationCrate*.

We then load the implementation dynamically at runtime:

*ApplicationCrate*

```rust
use savefile_abi::{AbiConnection};
use IntefaceCrate::{AdderInterface};

// Load the implementation of `dyn AdderInterface` that was published
// using the `savefile_abi_export!` above.
let connection = AbiConnection::<dyn AdderInterface>
        ::load_shared_library("ImplementationCrate.so").unwrap();

// The type `AbiConnection::<dyn AdderInterface>` implements
// the `AdderInterface`-trait, so we can use it to call its methods.
assert_eq!(connection.add(1, 2), 3);

```