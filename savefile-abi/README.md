# Welcome to Savefile-abi!

Full docs: https://docs.rs/savefile-abi/latest/

Savefile-abi is a crate that is primarily meant to help building binary plugins using Rust.


```toml
savefile-abi = "0.17"
savefile = "0.17"
savefile-derive = "0.17"
```

# Example

Let's say we have a crate that defines this trait for adding u32s:

*interface_crate*
```rust
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version=0)]
pub trait AdderInterface {
    fn add(&self, x: u32, y: u32) -> u32;
}

```

Now, we want to implement addition in a different crate, compile it to a shared library
(.dll or .so), and use it in the first crate (or some other crate):

*implementation_crate*
```rust
use interface_crate::{AdderInterface};
use savefile_derive::savefile_abi_export;

#[derive(Default)]
struct MyAdder { }

impl AdderInterface for MyAdder {
    fn add(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

// Export this implementation as the default-implementation for
// the interface 'AdderInterface', for the current library.
savefile_abi_export!(MyAdder, AdderInterface);

```

We add the following to Cargo.toml in our implementation crate:

```toml
[lib]
crate-type = ["cdylib"]
```

Now, in our application, we add a dependency to *interface_crate*, but not
to *ImplementationCrate*.

We then load the implementation dynamically at runtime:

*app*

```rust
use savefile_abi::{AbiConnection};
use interface_crate::{AdderInterface};


fn main() {
    // Load the implementation of `dyn AdderInterface` that was published
    // using the `savefile_abi_export!` above.
    let connection = AbiConnection::<dyn AdderInterface>
      ::load_shared_library("./ImplementationCrate.so").unwrap();

    // The type `AbiConnection::<dyn AdderInterface>` implements
    // the `AdderInterface`-trait, so we can use it to call its methods.
    assert_eq!(connection.add(1, 2), 3);
}

```

# Limitations

There are multiple limitations:

 * Tuples are presently not supported as direct function arguments!
 * There may be safety issues, Savefile-Abi is not mature yet.


See full docs: https://docs.rs/savefile-abi/latest/
