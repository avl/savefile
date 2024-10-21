#![allow(clippy::len_zero)]
#![deny(warnings)]
#![deny(missing_docs)]
#![allow(clippy::needless_late_init)]

/*!
This is the documentation for `savefile-abi`

# Welcome to savefile-abi!

Savefile-abi is a crate that is primarily meant to help building binary plugins using Rust.

Note! Savefile-abi now supports methods returning boxed futures! See the chapter 'async' below.

# Example

Let's say we have a crate that defines this trait for adding u32s:

*InterfaceCrate*
```
extern crate savefile_derive;
use savefile_derive::savefile_abi_exportable;

#[savefile_abi_exportable(version=0)]
pub trait AdderInterface {
    fn add(&self, x: u32, y: u32) -> u32;
}

```

Now, we want to implement addition in a different crate, compile it to a shared library
(.dll or .so), and use it in the first crate (or some other crate):

*ImplementationCrate*
```
 # extern crate savefile_derive;
 # use savefile_derive::{savefile_abi_exportable};
 # #[savefile_abi_exportable(version=0)]
 # pub trait AdderInterface {
 #   fn add(&self, x: u32, y: u32) -> u32;
 # }
 #
use savefile_derive::{savefile_abi_export};
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

```rust,no_run
 # extern crate savefile_derive;
 # mod adder_interface {
 #   use savefile_derive::savefile_abi_exportable;
 #   #[savefile_abi_exportable(version=0)]
 #   pub trait AdderInterface {
 #     fn add(&self, x: u32, y: u32) -> u32;
 #   }
 # }
 #
use adder_interface::AdderInterface;
use savefile_abi::AbiConnection;


// Load the implementation of `dyn AdderInterface` that was published
// using the `savefile_abi_export!` above.
let connection = AbiConnection::<dyn AdderInterface>
        ::load_shared_library("ImplementationCrate.so").unwrap();

// The type `AbiConnection::<dyn AdderInterface>` implements
// the `AdderInterface`-trait, so we can use it to call its methods.
assert_eq!(connection.add(1, 2), 3);

```

# More advanced examples

Interface containing closure arguments:
```
 # extern crate savefile_derive;
 # use savefile_derive::savefile_abi_exportable;
#[savefile_abi_exportable(version=0)]
pub trait CallMeBack {
    fn call_me(&self, x: &dyn Fn(u32) -> u32) -> u32;
    fn call_me_mut(&self, x: &mut dyn FnMut(u32) -> u32) -> u32;
}

```

Interface containing more complex types:
```
 # extern crate savefile_derive;
 # use savefile_derive::savefile_abi_exportable;
 # use std::collections::{HashMap, BinaryHeap};
#[savefile_abi_exportable(version=0)]
pub trait Processor {
    fn process(&self, x: &HashMap<String,String>, parameters: f32) -> BinaryHeap<u32>;
}

```

Interface containing user defined types:
```
 # extern crate savefile_derive;
 # use savefile_derive::{Savefile,savefile_abi_exportable};
 # use std::collections::{HashMap, BinaryHeap};

#[derive(Savefile)]
pub struct MyCustomType {
    pub name: String,
    pub age: u8,
    pub length: f32,
}

#[savefile_abi_exportable(version=0)]
pub trait Processor {
    fn insert(&self, x: &MyCustomType) -> Result<u32, String>;
}

```

# Versioning

Let's say the last example from the previous chapter needed to be evolved.
The type now needs a 'city' field.

We can add this while retaining compatibility with clients expecting the old API:

```
extern crate savefile_derive;

 # use savefile::prelude::SavefileError;
 # use savefile_derive::{Savefile,savefile_abi_exportable};
 # use savefile_abi::verify_compatiblity;
 # use std::collections::{HashMap, BinaryHeap};

#[derive(Savefile)]
pub struct MyCustomType {
    pub name: String,
    pub age: u8,
    pub length: f32,
    #[savefile_versions="1.."]
    pub city: String,
}

#[savefile_abi_exportable(version=1)]
pub trait Processor {
    fn insert(&self, x: &MyCustomType) -> Result<u32, String>;
}

#[cfg(test)]
{
    #[test]
    pub fn test_backward_compatibility() {
       // Automatically verify backward compatibility isn't broken.
       // Schemas for each version are stored in directory 'schemas',
       // and consulted on next run to ensure no change.
       // You should check the schemas in to source control.
       // If check fails for an unreleased version, just remove the schema file from
       // within 'schemas' directory.
       verify_compatiblity::<dyn Processor>("schemas").unwrap()
    }
}


```

Older clients, not aware of the 'city' field, can still call newer implementations. The 'city'
field will receive an empty string (Default::default()). Newer clients, calling older implementations,
will simply, automatically, omit the 'city' field.


# Background

Savefile-abi is a crate that is primarily meant to help building binary plugins using Rust.

The primary usecase is that a binary rust program is to be shipped to some customer,
who should then be able to load various binary modules into the program at runtime.
Savefile-abi defines ABI-stable rust-to-rust FFI for calling between a program and
a runtime-loaded shared library.

For now, both the main program and the plugins need to be written in rust. They can,
however, be written using different versions of the rust compiler, and the API may
be allowed to evolve. That is, data structures can be modified, and methods can be added
(or removed).

The reason savefile-abi is needed, is that rust does not have a stable 'ABI'. This means that
if shared libraries are built using rust, all libraries must be compiled by the same version of
rust, using the exact same source code. This means that rust cannot, 'out of the box', support
a binary plugin system, without something like savefile-abi. This restriction may be lifted
in the future, which would make this crate (savefile-abi) mostly redundant.

Savefile-abi does not solve the general 'stable ABI'-problem. Rather, it defines a limited
set of features, which allows useful calls between shared libraries, without allowing any
and all rust construct.

# Why another stable ABI-crate for Rust?

There are other crates also providing ABI-stability. Savefile-abi has the following properties:

 * It is able to completely specify the protocol used over the FFI-boundary. I.e, it can
   isolate two shared libraries completely, making minimal assumptions about data type
   memory layouts.

 * When it cannot prove that memory layouts are identical, it falls back to (fast) serialization.
   This has a performance penalty, and may require heap allocation.

 * It tries to require a minimum of configuration needed by the user, while still being safe.

 * It supports versioning of data structures (with a performance penalty).

 * It supports trait objects as arguments, including FnMut() and Fn().

 * Boxed trait objects, including Fn-traits, can be transferred across FFI-boundaries, passing
   ownership, safely. No unsafe code is needed by the user.

 * It requires enums to be `#[repr(uX)]` in order to pass them by reference. Other enums
   will still work correctly, but will be serialized under the hood at a performance penalty.

 * It places severe restrictions on types of arguments, since they must be serializable
   using the Savefile-crate for serialization. Basically, arguments must be 'simple', in that
   they must own all their contents, and be free of cycles. I.e, the type of the arguments must
   have lifetime `&'static`. Note, arguments may still be references, and the contents of the
   argument types may include Box, Vec etc, so this does not mean that only primitive types are
   supported.

Arguments cannot be mutable, since if serialization is needed, it would be impractical to detect and
handle updates to arguments made by the callee. This said, arguments can still have types such as
HashMap, IndexMap, Vec, String and custom defined structs or enums.

# How it all works

The basic principle is that savefile-abi makes sure to send function parameters in a way
that is certain to be understood by the code on the other end of the FFI-boundary.
It analyses if memory layouts of reference-parameters are the same on both sides of the
FFI-boundary, and if they are, the references are simply copied. In all other cases, including
all non-reference parameters, the data is simply serialized and sent as a binary buffer.

The callee cannot rely on any particular lifetimes of arguments, since if the arguments
were serialized, the arguments the callee sees will only have a lifetime of a single call,
regardless of the original lifetime. Savefile-abi inspects all lifetimes and ensures
that reference parameters don't have non-default lifetimes. Argument types must have static
lifetimes (otherwise they can't be serialized). The only exception is that the argument
can be reference types, but the type referenced must itself be `&'static`.

# About Safety

Savefile-Abi uses copious amounts of unsafe code. It has a test suite, and the
test suite passes with miri.

One thing to be aware of is that, at present, the AbiConnection::load_shared_library-method
is not marked as unsafe. However, if the .so-file given as argument is corrupt, using this
method can cause any amount of UB. Thus, it could be argued that it should be marked unsafe.

However, the same is true for _any_ shared library used by a rust program, including the
system C-library. It is also true that rust programs rely on the rust
compiler being implemented correctly. Thus, it has been
judged that the issue of corrupt binary files is beyond the scope of safety for Savefile-Abi.

As long as the shared library is a real Savefile-Abi shared library, it should be sound to use,
even if it contains code that is completely incompatible. This will be detected at runtime,
and either AbiConnection::load_shared_library will panic, or any calls made after will panic.

# About Vec and String references

Savefile-Abi allows passing references containing Vec and/or String across the FFI-boundary.
This is not normally guaranteed to be sound. However, Savefile-Abi uses heuristics to determine
the actual memory layout of both Vec and String, and verifies that the two libraries agree
on the layout. If they do not, the data is serialized instead. Also, since
parameters can never be mutable in Savefile-abi (except for closures), we know
the callee is not going to be freeing something allocated by the caller. Parameters
called by value are always serialized.

# Async

Savefile-abi now supports methods returning futures:

```rust

use savefile_derive::savefile_abi_exportable;
use std::pin::Pin;
use std::future::Future;
use std::time::Duration;

#[savefile_abi_exportable(version = 0)]
pub trait BoxedAsyncInterface {
    fn add_async(&mut self, x: u32, y: u32) -> Pin<Box<dyn Future<Output=String>>>;

}

struct SimpleImpl;

impl BoxedAsyncInterface for SimpleImpl {
    fn add_async(&mut self, x: u32, y: u32) -> Pin<Box<dyn Future<Output=String>>> {
        Box::pin(
            async move {
                /* any async code, using .await */
                format!("{}",x+y)
            }
        )
    }
}


```

It also supports the #[async_trait] proc macro crate. Use it like this:

```rust
use async_trait::async_trait;
use savefile_derive::savefile_abi_exportable;
use std::time::Duration;

#[async_trait]
#[savefile_abi_exportable(version = 0)]
pub trait SimpleAsyncInterface {
    async fn add_async(&mut self, x: u32, y: u32) -> u32;
}

struct SimpleImpl;

#[async_trait]
impl SimpleAsyncInterface for SimpleImpl {
    async fn add_async(&mut self, x: u32, y: u32) -> u32 {
        /* any async code, using .await */
        x + y
    }
}

```



*/

extern crate savefile;
extern crate savefile_derive;
use byteorder::ReadBytesExt;
use libloading::{Library, Symbol};
use savefile::{
    diff_schema, load_file_noschema, load_noschema, save_file_noschema, AbiMethodInfo, AbiTraitDefinition, Deserialize,
    Deserializer, LittleEndian, SavefileError, Schema, Serializer, CURRENT_SAVEFILE_LIB_VERSION,
};
use std::any::TypeId;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Cursor, Read, Write};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::panic::catch_unwind;
use std::path::Path;
use std::ptr::null;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Wake;
use std::{ptr, slice};

/// This trait is meant to be exported for a 'dyn SomeTrait'.
/// It can be automatically implemented by using the
/// macro `#[savefile_abi_exportable(version=0)]` on
/// a trait that is to be exportable.
///
/// NOTE!
/// If trait `MyExampleTrait` is to be exportable, the trait `AbiExportable` must
/// be implemented for `dyn MyExampleTrait`.
///
/// NOTE!
/// This trait is not meant to be implemented manually. It is mostly an implementation
/// detail of SavefileAbi, it is only ever meant to be implemented by the savefile-derive
/// proc macro.
///
/// # Safety
/// The implementor must:
///  * Make sure that the ABI_ENTRY function implements all parts of AbiProtocol
///    in a correct manner
///  * Has a correct 'get_definition' function, which must return a AbiTraitDefinition instance
///    that is truthful.
///  * Implement 'call' correctly
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be used across an ABI-boundary. Try adding a `#[savefile_abi_exportable(version=X)]` attribute to the declaration of the relevant trait.",
        label = "`{Self}` cannot be called across an ABI-boundary",
        note = "This error probably occurred because `{Self}` occurred as a return-value or argument to a method in a trait marked with `#[savefile_abi_exportable(version=X)]`, or because savefile_abi_export!-macro was used to export `{Self}`.",
    )
)]
pub unsafe trait AbiExportable {
    /// A function which implements the savefile-abi contract.
    const ABI_ENTRY: unsafe extern "C" fn(AbiProtocol);
    /// Must return a truthful description about all the methods in the
    /// `dyn trait` that AbiExportable is implemented for (i.e, `Self`).
    fn get_definition(version: u32) -> AbiTraitDefinition;
    /// Must return the current latest version of the interface. I.e,
    /// the version which Self represents. Of course, there may be future higher versions,
    /// but none such are known by the code.
    fn get_latest_version() -> u32;
    /// Implement method calling. Must deserialize data from 'data', and
    /// must return an outcome (result) by calling `receiver`.
    ///
    /// The return value is either Ok, or an error if the method to be called could
    /// not be found or for some reason not called (mismatched actual ABI, for example).
    ///
    /// `receiver` must be given 'abi_result' as its 'result_receiver' parameter, so that
    /// the receiver may set the result. The receiver executes at the caller-side of the ABI-divide,
    /// but it receives as first argument an RawAbiCallResult that has been created by the callee.
    fn call(
        trait_object: TraitObject,
        method_number: u16,
        effective_version: u32,
        compatibility_mask: u64,
        data: &[u8],
        abi_result: *mut (),
        receiver: unsafe extern "C" fn(
            outcome: *const RawAbiCallResult,
            result_receiver: *mut (), /* actual type: Result<T,SaveFileError>>*/
        ),
    ) -> Result<(), SavefileError>;
}

/// Trait that is to be implemented for the implementation of a trait whose `dyn trait` type
/// implements AbiExportable.
///
/// If `MyExampleTrait` is an ABI-exportable trait, and `MyExampleImplementation` is an
/// implementation of `MyExampleTrait`, then:
///  * The `AbiInterface` associated type must be `dyn MyExampleTrait`
///  * `AbiExportableImplementation` must be implemented for `MyExampleImplementation`
///
/// # Safety
/// The following must be fulfilled:
/// * ABI_ENTRY must be a valid function, implementing the AbiProtocol-protocol.
/// * AbiInterface must be 'dyn SomeTrait', where 'SomeTrait' is an exported trait.
///
#[cfg_attr(
    feature = "rust1_78",
    diagnostic::on_unimplemented(
        message = "`{Self}` cannot be the concrete type of an AbiExportable dyn trait.",
        label = "Does not implement `AbiExportableImplementation`",
        note = "You should not be using this trait directly, and should never see this error.",
    )
)]
pub unsafe trait AbiExportableImplementation {
    /// An entry point which implements the AbiProtocol protocol
    const ABI_ENTRY: unsafe extern "C" fn(AbiProtocol);
    /// The type 'dyn SomeTrait'.
    type AbiInterface: ?Sized + AbiExportable;
    /// A method which must be able to return a default-implementation of `dyn SomeTrait`.
    /// I.e, the returned box is a boxed dyn trait, not 'Self' (the actual implementation type).
    fn new() -> Box<Self::AbiInterface>;
}

/// Given a boxed trait object pointer, expressed as a data ptr and a vtable pointer,
/// of type T (which must be a `dyn SomeTrait` type), drop the boxed trait object.
/// I.e, `trait_object` is a type erased instance of Box<T> , where T is for example
/// `dyn MyTrait`.
/// # Safety
/// The given `trait_object` must be a boxed trait object.
unsafe fn destroy_trait_obj<T: AbiExportable + ?Sized>(trait_object: TraitObject) {
    let mut raw_ptr: MaybeUninit<*mut T> = MaybeUninit::uninit();
    ptr::copy(
        &trait_object as *const TraitObject as *const MaybeUninit<*mut T>,
        &mut raw_ptr as *mut MaybeUninit<*mut T>,
        1,
    );

    let _ = Box::from_raw(raw_ptr.assume_init());
}

/// Call the given method, on the trait object.
///
/// trait_object - Type erased version of Box<dyn SomeTrait>
/// method_number - The method to be called. This is an ordinal number, with 0 being the first method in definition order in the trait.
/// effective_version - The version number in the serialized format, negotiated previously.
/// compatibility_mask - For each method, one bit which says if the argument can be sent as just a reference, without having to use serialization to do a deep copy
/// data - All the arguments, in a slice
/// abi_result - Pointer to a place which will receiver the return value. This points to a Result<T, SaveFileError>, but since that type may have a different layout in callee and caller, we can't just use that type.
/// receiver - A function which will receiver the actual serialized return value, and an error code.
///
/// If the callee panics, this will be encoded into the RawAbiCallResult given to the `receiver`. The `reveiver` will always be called with the return value/return status.
///
/// # Safety
/// Every detail of all the arguments must be correct. Any little error is overwhelmingly likely to cause
/// a segfault or worse.
unsafe fn call_trait_obj<T: AbiExportable + ?Sized>(
    trait_object: TraitObject,
    method_number: u16,
    effective_version: u32,
    compatibility_mask: u64,
    data: &[u8],
    abi_result: *mut (),
    receiver: unsafe extern "C" fn(
        outcome: *const RawAbiCallResult,
        result_receiver: *mut (), /*Result<T,SaveFileError>>*/
    ),
) -> Result<(), SavefileError> {
    <T>::call(
        trait_object,
        method_number,
        effective_version,
        compatibility_mask,
        data,
        abi_result,
        receiver,
    )
}

/// Describes a method in a trait
#[derive(Debug)]
pub struct AbiConnectionMethod {
    /// The name of the method
    pub method_name: String,
    /// This is mostly for debugging, it's not actually used
    pub caller_info: AbiMethodInfo,
    /// The ordinal number of this method at the callee, or None if callee doesn't have
    /// method.
    pub callee_method_number: Option<u16>,
    /// For each of the up to 64 different arguments,
    /// a bit value of 1 means layout is identical, and in such a way that
    /// references can be just binary copied (owned arguments must still be cloned, and
    /// we can just as well do that using serialization, it will be approx as fast).
    pub compatibility_mask: u64,
}

/// Type erased carrier of a dyn trait fat pointer
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TraitObject {
    ptr: *const (),
    vtable: *const (),
}

unsafe impl Sync for TraitObject {}
unsafe impl Send for TraitObject {}

impl TraitObject {
    /// Returns a TraitObject with two null ptrs. This value must never be used,
    /// but can serve as a default before the real value is written.
    pub fn zero() -> TraitObject {
        TraitObject {
            ptr: null(),
            vtable: null(),
        }
    }

    /// Interpret this TraitObject as *mut T.
    /// *mut T *MUST* be a fat pointer of the same type as was used to create this TraitObject
    /// instance.
    pub fn as_mut_ptr<T: ?Sized>(self) -> *mut T {
        assert_eq!(
            std::mem::size_of::<*mut T>(),
            16,
            "TraitObject must only be used with dyn trait, not any other kind of trait"
        );

        let mut target: MaybeUninit<*mut T> = MaybeUninit::zeroed();
        unsafe {
            ptr::copy(
                &self as *const TraitObject as *const MaybeUninit<*mut T>,
                &mut target as *mut MaybeUninit<*mut T>,
                1,
            );
            target.assume_init()
        }
    }
    /// Interpret this TraitObject as *const T.
    /// *const T *MUST* be a fat pointer of the same type as was used to create this TraitObject
    /// instance.
    pub fn as_const_ptr<T: ?Sized>(self) -> *const T {
        assert_eq!(
            std::mem::size_of::<*const T>(),
            16,
            "TraitObject must only be used with dyn trait, not any other kind of trait"
        );

        let mut target: MaybeUninit<*const T> = MaybeUninit::zeroed();
        unsafe {
            ptr::copy(
                &self as *const TraitObject as *const MaybeUninit<*const T>,
                &mut target as *mut MaybeUninit<*const T>,
                1,
            );
            target.assume_init()
        }
    }
    /// Convert the given fat pointer to a TraitObject instance.
    pub fn new_from_ptr<T: ?Sized>(raw: *const T) -> TraitObject {
        assert_eq!(
            std::mem::size_of::<*const T>(),
            16,
            "TraitObject::new_from_ptr() must only be used with dyn trait, not any other kind of trait"
        );
        assert_eq!(std::mem::size_of::<TraitObject>(), 16);

        let mut trait_object = TraitObject::zero();

        unsafe {
            ptr::copy(
                &raw as *const *const T,
                &mut trait_object as *mut TraitObject as *mut *const T,
                1,
            )
        };
        trait_object
    }
    /// Note: This only works for boxed dyn Trait.
    /// T must be `dyn SomeTrait`.
    pub fn new<T: ?Sized>(input: Box<T>) -> TraitObject {
        let raw = Box::into_raw(input);
        assert_eq!(
            std::mem::size_of::<*mut T>(),
            16,
            "TraitObject::new() must only be used with Boxed dyn trait, not any other kind of Box"
        );
        assert_eq!(std::mem::size_of::<TraitObject>(), 16);

        let mut trait_object = TraitObject::zero();

        unsafe {
            ptr::copy(
                &raw as *const *mut T,
                &mut trait_object as *mut TraitObject as *mut *mut T,
                1,
            )
        };
        trait_object
    }
}

/// Information about an entry point and the trait
/// it corresponds to.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct AbiConnectionTemplate {
    /// The negotiated effective serialization version.
    /// See 'savefile' crate for more information about version handling.
    #[doc(hidden)]
    pub effective_version: u32,
    /// All the methods of the trait.
    #[doc(hidden)]
    pub methods: &'static [AbiConnectionMethod],
    /// The entry point which will actually be used for calls. Typically,
    /// this entry point points into a different shared object/dll compared to
    /// the caller.
    #[doc(hidden)]
    pub entry: unsafe extern "C" fn(flag: AbiProtocol),
}

/// Information about an ABI-connection.
///
/// I.e,
/// a caller and callee. The caller is in one
/// particular shared object, the callee in another.
/// Any modifiable state is stored in this object,
/// and the actual callee is stateless (allowing multiple
/// different incoming 'connections').
///
/// The fields are public, so that they can be easily written by the
/// proc macros. But the user should never interact with them directly,
/// so they are marked as doc(hidden).
#[repr(C)]
#[derive(Debug)]
pub struct AbiConnection<T: ?Sized> {
    /// Cachable information about the interface
    #[doc(hidden)]
    pub template: AbiConnectionTemplate,
    /// Information on whether we *own* the trait object.
    /// If we do, we must arrange for the foreign library code to drop it when we're done.
    /// Otherwise, we must not drop it.
    #[doc(hidden)]
    pub owning: Owning,
    /// The concrete trait object for this instance.
    /// I.e, type erased trait object in the foreign library
    #[doc(hidden)]
    pub trait_object: TraitObject,
    /// Phantom, to make this valid rust (since we don't otherwise carry a T).
    #[doc(hidden)]
    pub phantom: PhantomData<*const T>,
}
unsafe impl<T: ?Sized> Sync for AbiConnection<T> where T: Sync {}
unsafe impl<T: ?Sized> Send for AbiConnection<T> where T: Send {}

/// A trait object together with its entry point
#[repr(C)]
#[derive(Debug)]
pub struct PackagedTraitObject {
    /// Type erased trait object for an ABI-exported trait
    pub trait_object: TraitObject,
    /// The low level entry point
    pub entry: unsafe extern "C" fn(flag: AbiProtocol),
}

impl PackagedTraitObject {
    /// Create a PackagedTraitObject from a `Box<T>`    . T must be a trait object.
    /// T must implement AbiExportable, which means it has an ::ABI_ENTRY associated
    /// type that gives the entry point.
    pub fn new<T: AbiExportable + ?Sized>(boxed: Box<T>) -> PackagedTraitObject {
        let trait_object = TraitObject::new(boxed);
        let entry = T::ABI_ENTRY;
        PackagedTraitObject { trait_object, entry }
    }

    /// Create a PackagedTraitObject from a &T. T must be a trait object.
    /// T must implement AbiExportable, which means it has an ::ABI_ENTRY associated
    /// type that gives the entry point.
    /// Note, we use `*const T` here even for mutable cases, but it doesn't matter
    /// since it's never used, it's just cast to other stuff and then finally
    /// back to the right type.
    pub fn new_from_ptr<T>(r: *const T) -> PackagedTraitObject
    where
        T: AbiExportable + ?Sized,
    {
        assert_eq!(std::mem::size_of::<*const T>(), 16);
        let trait_object = TraitObject::new_from_ptr(r);
        let entry = T::ABI_ENTRY;
        PackagedTraitObject { trait_object, entry }
    }

    /// 'Serialize' this object. I.e, write it to a binary buffer, so that we can send it
    /// to a foreign library.
    pub fn serialize(self, serializer: &mut Serializer<impl Write>) -> Result<(), SavefileError> {
        serializer.write_ptr(self.trait_object.ptr)?;
        serializer.write_ptr(self.trait_object.vtable)?;
        serializer.write_ptr(self.entry as *const ())?;

        Ok(())
    }
    /// 'Deserialize' this object. I.e, read it from a binary buffer, so that we can receive it
    /// from a foreign library.
    ///
    /// # Safety
    /// The data available to read from Deserializer must be correct, and contain
    /// a valid serialized PackagedTraitObject.
    pub unsafe fn deserialize(
        deserializer: &mut Deserializer<impl Read>,
    ) -> Result<PackagedTraitObject, SavefileError> {
        let mut trait_object = TraitObject::zero();
        trait_object.ptr = deserializer.read_ptr()? as *mut ();
        trait_object.vtable = deserializer.read_ptr()? as *mut ();
        let entry = deserializer.read_ptr()? as *mut ();
        assert_eq!(std::mem::size_of::<unsafe extern "C" fn(flag: AbiProtocol)>(), 8);
        Ok(PackagedTraitObject {
            trait_object,
            entry: unsafe { std::mem::transmute::<*mut (), unsafe extern "C" fn(AbiProtocol)>(entry) },
        })
    }
}

impl<T: ?Sized> Drop for AbiConnection<T> {
    fn drop(&mut self) {
        match &self.owning {
            Owning::Owned => unsafe {
                (self.template.entry)(AbiProtocol::DropInstance {
                    trait_object: self.trait_object,
                });
            },
            Owning::NotOwned => {}
        }
    }
}

/// Helper struct carrying a pointer and length to an utf8 message.
/// We use this instead of &str, to guard against the hypothetical possibility
/// that the layout of &str would ever change.
#[repr(C)]
pub struct AbiErrorMsg {
    /// Pointer to utf8 error message
    pub error_msg_utf8: *const u8,
    /// The length of the message, in bytes
    pub len: usize,
}
impl AbiErrorMsg {
    /// Attempt to convert the given data to a String.
    /// Any invalid UTF8-chars are replaced.
    pub fn convert_to_string(&self) -> String {
        if self.len == 0 {
            return "".to_string();
        }
        let data = unsafe { slice::from_raw_parts(self.error_msg_utf8, self.len) };
        String::from_utf8_lossy(data).into()
    }
}

/// The result of calling a method in a foreign library.
#[repr(C, u8)]
pub enum RawAbiCallResult {
    /// Successful operation
    Success {
        /// A pointer to the return value, serialized
        data: *const u8,
        /// The size of the serialized return value, in bytes
        len: usize,
    },
    /// The method that was called, panicked. Since the way panic unwinding in Rust
    /// could change between rust-versions, we can't allow any panics to unwind
    /// across the boundary between two different libraries.
    Panic(AbiErrorMsg),
    /// There was an error in the ABI-framework. This will happen if code tries
    /// to call a method that is not actually available on the target, or if method
    /// signatures change in non ABI-compatible ways.
    AbiError(AbiErrorMsg),
}

/// This struct carries all information between different libraries.
/// I.e, it is the sole carrier of information accross an FFI-boundary.
#[repr(C, u8)]
pub enum AbiProtocol {
    /// Call a method on a trait object.
    RegularCall {
        /// Type-erased actual trait object. This is the 16 bytes o trait fat pointer.
        trait_object: TraitObject,
        /// For every argument, a bit '1' if said argument is a reference that can just
        /// be binary copied, as a pointer
        compatibility_mask: u64,
        /// Data for parameters, possibly serialized
        data: *const u8,
        /// Length of parameters-data
        data_length: usize,
        /// Instance of type `AbiCallResult<T>`, to which the return-value callback will
        /// write deserialized results or panic-message.
        abi_result: *mut (),
        /// Callback which will be called by callee in order to supply the return value
        /// (without having to allocate heap-memory)
        receiver: unsafe extern "C" fn(
            outcome: *const RawAbiCallResult,
            result_receiver: *mut (), /*Result<T,SaveFileError>>*/
        ),
        /// The negotiated protocol version
        effective_version: u32,
        /// The method to call. This is the method number using the
        /// numbering of the callee.
        method_number: u16,
    },
    /// Get callee version
    InterrogateVersion {
        /// The version of the callee savefile schema. This can only change if the savefile library
        /// is upgraded.
        schema_version_receiver: *mut u16,
        /// The version of the data schema, on the callee.
        abi_version_receiver: *mut u32,
    },
    /// Get schema
    InterrogateMethods {
        /// The version of the schema that the caller expects.
        schema_version_required: u16,
        /// The schema version that the caller expects the callee to communicate using.
        /// I.e, if callee has a later version of the 'savefile' library, this can be used
        /// to arrange for it to speak an older dialect. In theory, but savefile is still
        /// involving and there is always a risk that ABI-breaks will be necessary.
        callee_schema_version_interrogated: u32,
        /// A pointer pointing at the location that that caller will expect the return value to be written.
        /// Note, callee does not actually write to this, it just calls `callback`, which allows caller
        /// to write to the result_receiver. The field is still needed here, since the `callback` is a bare function,
        /// and cannot capture any data.
        result_receiver: *mut (), /*Result<AbiTraitDefinition, SavefileError>*/
        /// Called by callee to convey information back to caller.
        /// `receiver` is place the caller will want to write the result.
        callback: unsafe extern "C" fn(
            receiver: *mut (), /*Result<AbiTraitDefinition, SavefileError>*/
            callee_schema_version: u16,
            data: *const u8,
            len: usize,
        ),
    },
    /// Create a new trait object.
    CreateInstance {
        /// Pointer which will receive the fat pointer to the dyn trait object, allocated on heap using Box.
        trait_object_receiver: *mut TraitObject,
        /// Opaque pointer to callers representation of error (String)
        error_receiver: *mut (), /*String*/
        /// Called by callee if instance creation fails (by panic)
        error_callback: unsafe extern "C" fn(error_receiver: *mut (), error: *const AbiErrorMsg),
    },
    /// Drop a trait object.
    DropInstance {
        /// dyn trait fat pointer
        trait_object: TraitObject,
    },
}

/// Parse the given RawAbiCallResult. If it concerns a success, then deserialize a return value using the given closure.
pub fn parse_return_value_impl<T>(
    outcome: &RawAbiCallResult,
    deserialize_action: impl FnOnce(&mut Deserializer<Cursor<&[u8]>>) -> Result<T, SavefileError>,
) -> Result<T, SavefileError> {
    match outcome {
        RawAbiCallResult::Success { data, len } => {
            let data = unsafe { std::slice::from_raw_parts(*data, *len) };
            let mut reader = Cursor::new(data);
            let file_version = reader.read_u32::<LittleEndian>()?;
            let mut deserializer = Deserializer {
                reader: &mut reader,
                file_version,
                ephemeral_state: HashMap::new(),
            };
            deserialize_action(&mut deserializer)
            //T::deserialize(&mut deserializer)
        }
        RawAbiCallResult::Panic(AbiErrorMsg { error_msg_utf8, len }) => {
            let errdata = unsafe { std::slice::from_raw_parts(*error_msg_utf8, *len) };
            Err(SavefileError::CalleePanic {
                msg: String::from_utf8_lossy(errdata).into(),
            })
        }
        RawAbiCallResult::AbiError(AbiErrorMsg { error_msg_utf8, len }) => {
            let errdata = unsafe { std::slice::from_raw_parts(*error_msg_utf8, *len) };
            Err(SavefileError::GeneralError {
                msg: String::from_utf8_lossy(errdata).into(),
            })
        }
    }
}

/// Parse an RawAbiCallResult instance into a `Result<Box<dyn T>, SavefileError>` .
///
/// This is used on the caller side, and the type T will always be statically known.
/// TODO: There's some duplicated code here, compare parse_return_value
pub fn parse_return_boxed_trait<T>(outcome: &RawAbiCallResult) -> Result<Box<AbiConnection<T>>, SavefileError>
where
    T: AbiExportable + ?Sized + 'static,
{
    parse_return_value_impl(outcome, |deserializer| {
        let packaged = unsafe { PackagedTraitObject::deserialize(deserializer)? };
        unsafe {
            Ok(Box::new(AbiConnection::<T>::from_raw_packaged(
                packaged,
                Owning::Owned,
            )?))
        }
    })
}
/// We never unload libraries which have been dynamically loaded, because of all the problems with
/// doing so.
static LIBRARY_CACHE: Mutex<Option<HashMap<String /*filename*/, Library>>> = Mutex::new(None);
static ENTRY_CACHE: Mutex<
    Option<HashMap<(String /*filename*/, String /*trait name*/), unsafe extern "C" fn(flag: AbiProtocol)>>,
> = Mutex::new(None);

static ABI_CONNECTION_TEMPLATES: Mutex<
    Option<HashMap<(TypeId, unsafe extern "C" fn(flag: AbiProtocol)), AbiConnectionTemplate>>,
> = Mutex::new(None);

struct Guard<'a, K: Hash + Eq, V> {
    guard: MutexGuard<'a, Option<HashMap<K, V>>>,
}

impl<K: Hash + Eq, V> std::ops::Deref for Guard<'_, K, V> {
    type Target = HashMap<K, V>;
    fn deref(&self) -> &HashMap<K, V> {
        self.guard.as_ref().unwrap()
    }
}

impl<K: Hash + Eq, V> std::ops::DerefMut for Guard<'_, K, V> {
    fn deref_mut(&mut self) -> &mut HashMap<K, V> {
        &mut *self.guard.as_mut().unwrap()
    }
}

// Avoid taking a dependency on OnceCell or lazy_static or something, just for this little thing
impl<'a, K: Hash + Eq, V> Guard<'a, K, V> {
    pub fn lock(map: &'a Mutex<Option<HashMap<K /*filename*/, V>>>) -> Guard<'a, K, V> {
        let mut guard = map.lock().unwrap();
        if guard.is_none() {
            *guard = Some(HashMap::new());
        }
        Guard { guard }
    }
}

/// Helper to determine if something is owned, or not
#[derive(Debug, Clone, Copy)]
pub enum Owning {
    /// The object is owned
    Owned,
    /// The object is not owned
    NotOwned,
}

const FLEX_BUFFER_SIZE: usize = 64;
/// Stack allocated buffer that overflows on heap if needed
#[doc(hidden)]
pub enum FlexBuffer {
    /// Allocated on stack>
    Stack {
        /// The current write position. This is the same as
        /// the logical size of the buffer, since we can only write at the end.
        position: usize,
        /// The data backing this buffer, on the stack
        data: MaybeUninit<[u8; FLEX_BUFFER_SIZE]>,
    },
    /// Allocated on heap
    Spill(Vec<u8>),
}
impl Write for FlexBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            FlexBuffer::Stack { position, data } => {
                if *position + buf.len() <= FLEX_BUFFER_SIZE {
                    let rawdata = data as *mut MaybeUninit<_> as *mut u8;
                    unsafe { ptr::copy(buf.as_ptr(), rawdata.add(*position), buf.len()) };
                    *position += buf.len();
                } else {
                    let mut spill = Vec::with_capacity(2 * FLEX_BUFFER_SIZE + buf.len());
                    let rawdata = data as *mut MaybeUninit<_> as *mut u8;
                    let dataslice = unsafe { slice::from_raw_parts(rawdata, *position) };
                    spill.extend(dataslice);
                    spill.extend(buf);
                    *self = FlexBuffer::Spill(spill);
                }
            }
            FlexBuffer::Spill(v) => v.extend(buf),
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Raw entry point for receiving return values from other shared libraries
#[doc(hidden)]
pub unsafe extern "C" fn abi_result_receiver<T: Deserialize>(
    outcome: *const RawAbiCallResult,
    result_receiver: *mut (),
) {
    let outcome = unsafe { &*outcome };
    let result_receiver = unsafe { &mut *(result_receiver as *mut std::mem::MaybeUninit<Result<T, SavefileError>>) };
    result_receiver.write(parse_return_value_impl(outcome, |deserializer| {
        T::deserialize(deserializer)
    }));
}

/// Raw entry point for receiving return values from other shared libraries
#[doc(hidden)]
pub unsafe extern "C" fn abi_boxed_trait_receiver<T>(outcome: *const RawAbiCallResult, result_receiver: *mut ())
where
    T: AbiExportable + ?Sized + 'static,
{
    let outcome = unsafe { &*outcome };
    let result_receiver =
        unsafe { &mut *(result_receiver as *mut std::mem::MaybeUninit<Result<Box<AbiConnection<T>>, SavefileError>>) };
    result_receiver.write(parse_return_value_impl(outcome, |deserializer| {
        let packaged = unsafe { PackagedTraitObject::deserialize(deserializer)? };
        unsafe {
            Ok(Box::new(AbiConnection::<T>::from_raw_packaged(
                packaged,
                Owning::Owned,
            )?))
        }
    }));
}

// Flex buffer is only used internally, and we don't need to provide
// any of the regular convenience.
#[allow(clippy::new_without_default)]
#[allow(clippy::len_without_is_empty)]
impl FlexBuffer {
    /// Create a new buffer instance, allocated from the stack
    pub fn new() -> FlexBuffer {
        FlexBuffer::Stack {
            position: 0,
            data: MaybeUninit::uninit(),
        }
    }
    /// Get a pointer to the buffer contents
    pub fn as_ptr(&self) -> *const u8 {
        match self {
            FlexBuffer::Stack { data, .. } => data as *const MaybeUninit<_> as *const u8,
            FlexBuffer::Spill(v) => v.as_ptr(),
        }
    }
    /// Get the number of bytes in the buffer
    pub fn len(&self) -> usize {
        match self {
            FlexBuffer::Stack { position, .. } => *position,
            FlexBuffer::Spill(v) => v.len(),
        }
    }
}

/// Arguments are layout compatible if their native versions are layout_compatible,
/// or if they are traits and the effective version of the traits are compatible.
/// For traits, the actual fat pointer is always compatible, so can always be used.
/// The trait-objects themselves can never be serialized, so they can only be used as references.
///
/// b is the callee, a is the caller
fn arg_layout_compatible(
    a_native: &Schema,
    b_native: &Schema,
    a_effective: &Schema,
    b_effective: &Schema,
    effective_version: u32,
    is_return_position: bool,
) -> Result<bool, SavefileError> {
    match (a_native, b_native) {
        (Schema::Future(_a, _, _, _), Schema::Future(_b, _, _, _)) => {
            let (
                Schema::Future(effective_a2, a_send, a_sync, a_unpin),
                Schema::Future(effective_b2, b_send, b_sync, b_unpin),
            ) = (a_effective, b_effective)
            else {
                return Err(SavefileError::IncompatibleSchema {
                    message: "Type has changed".to_string(),
                });
            };
            for (a, b, bound) in [
                (*a_send, *b_send, "Send"),
                (*a_sync, *b_sync, "Sync"),
                (*a_unpin, *b_unpin, "Unpin"),
            ] {
                if a && !b {
                    return Err(SavefileError::IncompatibleSchema{message: format!(
                        "Caller expects a future with an {}-bound, but implementation provides one without. This is an incompatible difference.",
                         bound)
                    });
                }
            }

            effective_a2.verify_backward_compatible(effective_version, effective_b2, is_return_position)?;
            Ok(true)
        }
        (Schema::FnClosure(a1, _a2), Schema::FnClosure(b1, _b2)) => {
            let (Schema::FnClosure(effective_a1, effective_a2), Schema::FnClosure(effective_b1, effective_b2)) =
                (a_effective, b_effective)
            else {
                return Err(SavefileError::IncompatibleSchema {
                    message: "Type has changed".to_string(),
                });
            };

            effective_a2.verify_backward_compatible(effective_version, effective_b2, is_return_position)?;
            Ok(a1 == b1 && a1 == effective_a1 && a1 == effective_b1)
        }
        (Schema::Boxed(native_a), Schema::Boxed(native_b)) => {
            let (Schema::Boxed(effective_a2), Schema::Boxed(effective_b2)) = (a_effective, b_effective) else {
                return Err(SavefileError::IncompatibleSchema {
                    message: "Type has changed".to_string(),
                });
            };
            arg_layout_compatible(
                native_a,
                native_b,
                effective_a2,
                effective_b2,
                effective_version,
                is_return_position,
            )
        }
        (Schema::Trait(s_a, _), Schema::Trait(s_b, _)) => {
            if s_a != s_b {
                return Err(SavefileError::IncompatibleSchema {
                    message: "Type has changed".to_string(),
                });
            }
            let (Schema::Trait(e_a2, effective_a2), Schema::Trait(e_b2, effective_b2)) = (a_effective, b_effective)
            else {
                return Err(SavefileError::IncompatibleSchema {
                    message: "Type has changed".to_string(),
                });
            };
            if e_a2 != e_b2 {
                return Err(SavefileError::IncompatibleSchema {
                    message: "Type has changed".to_string(),
                });
            }

            effective_a2.verify_backward_compatible(effective_version, effective_b2, is_return_position)?;
            Ok(true)
        }
        (a, b) => Ok(a.layout_compatible(b)),
    }
}

impl<T: AbiExportable + ?Sized + 'static> AbiConnection<T> {
    /// Analyse the difference in definitions between the two sides,
    /// and create an AbiConnection
    #[allow(clippy::too_many_arguments)]
    fn analyze_and_create(
        trait_name: &str,
        remote_entry: unsafe extern "C" fn(flag: AbiProtocol),
        effective_version: u32,
        caller_effective_definition: AbiTraitDefinition,
        callee_effective_definition: AbiTraitDefinition,
        caller_native_definition: AbiTraitDefinition,
        callee_native_definition: AbiTraitDefinition,
    ) -> Result<AbiConnectionTemplate, SavefileError> {
        let mut methods = Vec::with_capacity(caller_native_definition.methods.len());
        if caller_native_definition.methods.len() > 64 {
            panic!("Too many method arguments, max 64 are supported!");
        }
        for caller_native_method in caller_native_definition.methods.into_iter() {
            let Some((callee_native_method_number, callee_native_method)) = callee_native_definition
                .methods
                .iter()
                .enumerate()
                .find(|x| x.1.name == caller_native_method.name)
            else {
                methods.push(AbiConnectionMethod {
                    method_name: caller_native_method.name,
                    caller_info: caller_native_method.info,
                    callee_method_number: None,
                    compatibility_mask: 0,
                });
                continue;
            };

            let Some(callee_effective_method) = callee_effective_definition
                .methods
                .iter()
                .find(|x| x.name == caller_native_method.name)
            else {
                return Err(SavefileError::GeneralError {msg: format!("Internal error - missing method definition {} in signature when calculating serializable version of call (1).", caller_native_method.name)});
            };

            let Some(caller_effective_method) = caller_effective_definition
                .methods
                .iter()
                .find(|x| x.name == caller_native_method.name)
            else {
                return Err(SavefileError::GeneralError {msg: format!("Internal error - missing method definition {} in signature when calculating serializable version of call (2).", caller_native_method.name)});
            };

            if caller_native_method.info.arguments.len() != callee_native_method.info.arguments.len() {
                return Err(SavefileError::GeneralError {msg: format!("Number of arguments for method {} was expected by caller to be {} but was {} in implementation.", caller_native_method.name, caller_native_method.info.arguments.len(), callee_native_method.info.arguments.len())});
            }

            if caller_native_method.info.arguments.len() != caller_effective_method.info.arguments.len() {
                return Err(SavefileError::GeneralError {
                    msg: format!(
                        "Internal error - number of arguments for method {} has differs between {} to {} (1).",
                        caller_native_method.name,
                        caller_native_method.info.arguments.len(),
                        caller_effective_method.info.arguments.len()
                    ),
                });
            }

            if caller_native_method.info.arguments.len() != callee_effective_method.info.arguments.len() {
                return Err(SavefileError::GeneralError {
                    msg: format!(
                        "Internal error - number of arguments for method {} has differs between {} to {} (2).",
                        caller_native_method.name,
                        caller_native_method.info.arguments.len(),
                        callee_effective_method.info.arguments.len()
                    ),
                });
            }

            if caller_native_method.info.arguments.len() > 64 {
                return Err(SavefileError::TooManyArguments);
            }

            let retval_effective_schema_diff = diff_schema(
                &caller_effective_method.info.return_value,
                &callee_effective_method.info.return_value,
                "".to_string(),
                true,
            );
            if let Some(diff) = retval_effective_schema_diff {
                return Err(SavefileError::IncompatibleSchema {
                    message: format!(
                        "Incompatible ABI detected. Trait: {}, method: {}, return value error: {}",
                        trait_name, &caller_native_method.name, diff
                    ),
                });
            }
            let mut mask = 0;
            let mut verify_compatibility = |effective1, effective2, native1, native2, index: Option<usize>| {
                let effective_schema_diff = diff_schema(effective1, effective2, "".to_string(), index.is_none());
                if let Some(diff) = effective_schema_diff {
                    return Err(SavefileError::IncompatibleSchema {
                        message: if let Some(index) = index {
                            format!(
                                "Incompatible ABI detected. Trait: {}, method: {}, argument: #{}: {}",
                                trait_name, &caller_native_method.name, index, diff
                            )
                        } else {
                            format!(
                                "Incompatible ABI detected. Trait: {}, method: {}, return value differs: {}",
                                trait_name, &caller_native_method.name, diff
                            )
                        },
                    });
                }

                let comp = arg_layout_compatible(
                    native1,
                    native2,
                    effective1,
                    effective2,
                    effective_version,
                    index.is_none(),
                )?;

                if comp {
                    if let Some(index) = index {
                        mask |= 1 << index;
                    }
                }
                Ok(())
            };

            for index in 0..caller_native_method.info.arguments.len() {
                let effective1 = &caller_effective_method.info.arguments[index].schema;
                let effective2 = &callee_effective_method.info.arguments[index].schema;
                let native1 = &caller_native_method.info.arguments[index].schema;
                let native2 = &callee_native_method.info.arguments[index].schema;
                verify_compatibility(effective1, effective2, native1, native2, Some(index))?;
            }

            verify_compatibility(
                &caller_effective_method.info.return_value,
                &callee_effective_method.info.return_value,
                &caller_native_method.info.return_value,
                &callee_native_method.info.return_value,
                None, /*return value*/
            )?;

            methods.push(AbiConnectionMethod {
                method_name: caller_native_method.name,
                caller_info: caller_native_method.info,
                callee_method_number: Some(callee_native_method_number as u16),
                compatibility_mask: mask,
            })
        }

        Ok(AbiConnectionTemplate {
            effective_version,
            methods: Box::leak(methods.into_boxed_slice()),
            entry: remote_entry,
        })
    }

    /// Gets the function pointer for the entry point of the given interface, in the given
    /// shared library.
    fn get_symbol_for(
        shared_library_path: &str,
        trait_name: &str,
    ) -> Result<unsafe extern "C" fn(flag: AbiProtocol), SavefileError> {
        let mut entry_guard = Guard::lock(&ENTRY_CACHE);
        let mut lib_guard = Guard::lock(&LIBRARY_CACHE);

        if let Some(item) = entry_guard.get(&(shared_library_path.to_string(), trait_name.to_string())) {
            return Ok(*item);
        }

        let filename = shared_library_path.to_string();
        let trait_name = trait_name.to_string();
        let library;
        match lib_guard.entry(filename.clone()) {
            Entry::Occupied(item) => {
                library = item.into_mut();
            }
            Entry::Vacant(vacant) => unsafe {
                library = vacant.insert(Library::new(&filename).map_err(|x| SavefileError::LoadLibraryFailed {
                    libname: filename.to_string(),
                    msg: x.to_string(),
                })?);
            },
        }

        match entry_guard.entry((filename.clone(), trait_name.clone())) {
            Entry::Occupied(item) => Ok(*item.get()),
            Entry::Vacant(vacant) => {
                let symbol_name = format!("abi_entry_{}\0", trait_name);
                let symbol: Symbol<unsafe extern "C" fn(flag: AbiProtocol)> = unsafe {
                    library
                        .get(symbol_name.as_bytes())
                        .map_err(|x| SavefileError::LoadSymbolFailed {
                            libname: filename.to_string(),
                            symbol: symbol_name,
                            msg: x.to_string(),
                        })?
                };
                let func: unsafe extern "C" fn(flag: AbiProtocol) =
                    unsafe { std::mem::transmute(symbol.into_raw().into_raw()) };
                vacant.insert(func);
                Ok(func)
            }
        }
    }

    /// Determines the name, without namespace, of the implemented
    /// trait.
    fn trait_name() -> &'static str {
        let n = std::any::type_name::<T>();
        let n = n.split("::").last().unwrap();
        n
    }
    /// Load the shared library given by 'filename', and find a savefile-abi-implementation of
    /// the trait 'T'. Returns an object that implements the
    ///
    /// # Safety
    /// The shared library referenced by 'filename' must be safely implemented,
    /// and must contain an ABI-exported implementation of T, which must be a dyn trait.
    /// However, this kind of guarantee is really needed for all execution of any rust code,
    /// so we don't mark this as unsafe. Symbols are unlikely to match by mistake.
    pub fn load_shared_library(filename: &str) -> Result<AbiConnection<T>, SavefileError> {
        let remote_entry = Self::get_symbol_for(filename, Self::trait_name())?;
        Self::new_internal(remote_entry, None, Owning::Owned)
    }

    /// Creates an AbiConnection from a PackagedTraitObject
    /// This is the way the derive macro crates AbiConnection instances.
    ///
    /// # Safety
    /// * entry_point of `packed` must implement AbiProtocol
    /// * trait_object of `packed` must be a type erased trait object reference
    /// * owning must be correct
    #[doc(hidden)]
    pub unsafe fn from_raw_packaged(
        packed: PackagedTraitObject,
        owning: Owning,
    ) -> Result<AbiConnection<T>, SavefileError> {
        Self::from_raw(packed.entry, packed.trait_object, owning)
    }

    /// Check if the given argument 'arg' in method 'method' is memory compatible such that
    /// it will be sent as a reference, not copied. This will depend on the memory layout
    /// of the code being called into. It will not change during the lifetime of an
    /// AbiConnector, but it may change if the target library is recompiled.
    pub fn get_arg_passable_by_ref(&self, method: &str, arg: usize) -> bool {
        if let Some(found) = self.template.methods.iter().find(|var| var.method_name == method) {
            let abi_method: &AbiConnectionMethod = found;
            if arg >= abi_method.caller_info.arguments.len() {
                panic!(
                    "Method '{}' has only {} arguments, so there is no argument #{}",
                    method,
                    abi_method.caller_info.arguments.len(),
                    arg
                );
            }
            (abi_method.compatibility_mask & (1 << (arg as u64))) != 0
        } else {
            let arg_names: Vec<_> = self.template.methods.iter().map(|x| x.method_name.as_str()).collect();
            panic!(
                "Trait has no method with name '{}'. Available methods: {}",
                method,
                arg_names.join(", ")
            );
        }
    }

    /// This routine is mostly for tests.
    /// It allows using a raw external API entry point directly.
    /// This is mostly useful for internal testing of the savefile-abi-library.
    /// 'miri' does not support loading dynamic libraries. Using this function
    /// from within the same image as the implementation, can be a workaround for this.
    ///
    /// # Safety
    /// * entry_point must implement AbiProtocol
    /// * trait_object must be a type erased trait object reference
    /// * owning must be correct
    #[doc(hidden)]
    pub unsafe fn from_raw(
        entry_point: unsafe extern "C" fn(AbiProtocol),
        trait_object: TraitObject,
        owning: Owning,
    ) -> Result<AbiConnection<T>, SavefileError> {
        Self::new_internal(entry_point, Some(trait_object), owning)
    }

    /// Crate a AbiConnection from an entry point and a boxed trait object.
    /// This is undocumented, since it's basically useless except for tests.
    /// If you have a Box<dyn Example>, you'd want to just use it directly,
    /// not make an AbiConnection wrapping it.
    ///
    /// This method is still useful during testing.
    ///
    /// # Safety
    ///  * The entry point must contain a correct implementation matching the type T.
    ///  * T must be a dyn trait object
    #[doc(hidden)]
    pub fn from_boxed_trait(trait_object: Box<T>) -> Result<AbiConnection<T>, SavefileError> {
        let trait_object = TraitObject::new(trait_object);
        Self::new_internal(T::ABI_ENTRY, Some(trait_object), Owning::Owned)
    }

    /// Crate a AbiConnection from an entry point and a boxed trait object.
    /// This allows using a different interface trait for the backing implementation, for
    /// test cases which want to test version evolution.
    ///
    /// # Safety
    ///  * The entry point must contain a correct implementation matching the type T.
    ///  * T must be a dyn trait object
    #[doc(hidden)]
    pub unsafe fn from_boxed_trait_for_test<O: AbiExportable + ?Sized>(
        entry_point: unsafe extern "C" fn(AbiProtocol),
        trait_object: Box<O>,
    ) -> Result<AbiConnection<T>, SavefileError> {
        let trait_object = TraitObject::new(trait_object);
        Self::new_internal(entry_point, Some(trait_object), Owning::Owned)
    }

    fn new_internal(
        remote_entry: unsafe extern "C" fn(AbiProtocol),
        trait_object: Option<TraitObject>,
        owning: Owning,
    ) -> Result<AbiConnection<T>, SavefileError> {
        let mut templates = Guard::lock(&ABI_CONNECTION_TEMPLATES);

        let typeid = TypeId::of::<T>();
        // In principle, it would be enough to key 'templates' based on 'remote_entry'.
        // However, if we do, and the user ever uses AbiConnection<T> with the _wrong_ entry point,
        // we risk poisoning the cache with erroneous data.
        let template = match templates.entry((typeid, remote_entry)) {
            Entry::Occupied(template) => template.get().clone(),
            Entry::Vacant(vacant) => {
                let own_version = T::get_latest_version();
                let own_native_definition = T::get_definition(own_version);

                let mut callee_abi_version = 0u32;
                let mut callee_schema_version = 0u16;
                unsafe {
                    (remote_entry)(AbiProtocol::InterrogateVersion {
                        schema_version_receiver: &mut callee_schema_version as *mut _,
                        abi_version_receiver: &mut callee_abi_version as *mut _,
                    });
                }

                let effective_schema_version = callee_schema_version.min(CURRENT_SAVEFILE_LIB_VERSION);
                let effective_version = own_version.min(callee_abi_version);

                let mut callee_abi_native_definition = Err(SavefileError::ShortRead); //Uust dummy-values
                let mut callee_abi_effective_definition = Err(SavefileError::ShortRead);

                unsafe extern "C" fn definition_receiver(
                    receiver: *mut (), //Result<AbiTraitDefinition, SavefileError>,
                    schema_version: u16,
                    data: *const u8,
                    len: usize,
                ) {
                    let receiver = unsafe { &mut *(receiver as *mut Result<AbiTraitDefinition, SavefileError>) };
                    let slice = unsafe { slice::from_raw_parts(data, len) };
                    let mut cursor = Cursor::new(slice);

                    *receiver = load_noschema(&mut cursor, schema_version.into());
                }

                unsafe {
                    (remote_entry)(AbiProtocol::InterrogateMethods {
                        schema_version_required: effective_schema_version,
                        callee_schema_version_interrogated: callee_abi_version,
                        result_receiver: &mut callee_abi_native_definition as *mut _ as *mut _,
                        callback: definition_receiver,
                    });
                }

                unsafe {
                    (remote_entry)(AbiProtocol::InterrogateMethods {
                        schema_version_required: effective_schema_version,
                        callee_schema_version_interrogated: effective_version,
                        result_receiver: &mut callee_abi_effective_definition as *mut _ as *mut _,
                        callback: definition_receiver,
                    });
                }

                let callee_abi_native_definition = callee_abi_native_definition?;
                let callee_abi_effective_definition = callee_abi_effective_definition?;

                let own_effective_definition = T::get_definition(effective_version);
                let trait_name = Self::trait_name();
                let template = Self::analyze_and_create(
                    trait_name,
                    remote_entry,
                    effective_version,
                    own_effective_definition,
                    callee_abi_effective_definition,
                    own_native_definition,
                    callee_abi_native_definition,
                )?;
                vacant.insert(template).clone()
            }
        };

        let trait_object = if let Some(obj) = trait_object {
            obj
        } else {
            let mut trait_object = TraitObject::zero();
            let mut error_msg: String = Default::default();
            unsafe extern "C" fn error_callback(error_receiver: *mut (), error: *const AbiErrorMsg) {
                let error_msg = unsafe { &mut *(error_receiver as *mut String) };
                *error_msg = unsafe { &*error }.convert_to_string();
            }
            unsafe {
                (remote_entry)(AbiProtocol::CreateInstance {
                    trait_object_receiver: &mut trait_object as *mut _,
                    error_receiver: &mut error_msg as *mut String as *mut _,
                    error_callback,
                });
            }

            if error_msg.len() > 0 {
                return Err(SavefileError::CalleePanic { msg: error_msg });
            }
            trait_object
        };

        Ok(AbiConnection {
            template,
            owning,
            trait_object,
            phantom: PhantomData,
        })
    }
}

/// Helper implementation of ABI entry point.
/// The actual low level `extern "C"` functions call into this.
/// This is an entry point meant to be used by the derive macro.
///
/// This version, the 'light version', does not support instance
/// creation.
///
/// # Safety
/// The 'AbiProtocol' protocol must only contain valid data.
pub unsafe fn abi_entry_light<T: AbiExportable + ?Sized>(flag: AbiProtocol) {
    match flag {
        AbiProtocol::RegularCall {
            trait_object,
            method_number,
            effective_version,
            compatibility_mask,
            data,
            data_length,
            abi_result,
            receiver,
        } => {
            let result = catch_unwind(|| {
                let data = unsafe { slice::from_raw_parts(data, data_length) };

                match unsafe {
                    call_trait_obj::<T>(
                        trait_object,
                        method_number,
                        effective_version,
                        compatibility_mask,
                        data,
                        abi_result,
                        receiver,
                    )
                } {
                    Ok(_) => {}
                    Err(err) => {
                        let msg = format!("{:?}", err);
                        let err = RawAbiCallResult::AbiError(AbiErrorMsg {
                            error_msg_utf8: msg.as_ptr(),
                            len: msg.len(),
                        });
                        receiver(&err, abi_result)
                    }
                }
            });
            match result {
                Ok(()) => {}
                Err(err) => {
                    let msg: &str;
                    let temp;
                    if let Some(err) = err.downcast_ref::<&str>() {
                        msg = err;
                    } else {
                        temp = format!("{:?}", err);
                        msg = &temp;
                    }
                    let err = RawAbiCallResult::Panic(AbiErrorMsg {
                        error_msg_utf8: msg.as_ptr(),
                        len: msg.len(),
                    });
                    receiver(&err, abi_result)
                }
            }
        }
        AbiProtocol::InterrogateVersion {
            schema_version_receiver,
            abi_version_receiver,
        } => {
            // # SAFETY
            // The pointers come from another savefile-implementation, and are known to be valid
            unsafe {
                *schema_version_receiver = CURRENT_SAVEFILE_LIB_VERSION;
                *abi_version_receiver = <T as AbiExportable>::get_latest_version();
            }
        }
        AbiProtocol::InterrogateMethods {
            schema_version_required,
            callee_schema_version_interrogated,
            result_receiver,
            callback,
        } => {
            // Note! Any conforming implementation must send a 'schema_version_required' number that is
            // within the ability of the receiving implementation. It can interrogate this using 'AbiProtocol::InterrogateVersion'.
            let abi = <T as AbiExportable>::get_definition(callee_schema_version_interrogated);
            let mut temp = vec![];
            let Ok(_) = Serializer::save_noschema_internal(
                &mut temp,
                schema_version_required as u32,
                &abi,
                schema_version_required.min(CURRENT_SAVEFILE_LIB_VERSION),
            ) else {
                return;
            };
            callback(result_receiver, schema_version_required, temp.as_ptr(), temp.len());
        }
        AbiProtocol::CreateInstance {
            trait_object_receiver: _,
            error_receiver,
            error_callback,
        } => {
            let msg = format!("Internal error - attempt to create an instance of {} using the interface crate, not an implementation crate", std::any::type_name::<T>());
            let err = AbiErrorMsg {
                error_msg_utf8: msg.as_ptr(),
                len: msg.len(),
            };
            error_callback(error_receiver, &err as *const _)
        }
        AbiProtocol::DropInstance { trait_object } => unsafe {
            destroy_trait_obj::<T>(trait_object);
        },
    }
}
/// Helper implementation of ABI entry point.
/// The actual low level `extern "C"` functions call into this.
/// This is an entry point meant to be used by the derive macro.
///
/// This version, the 'full version', does support instance
/// creation.
///
/// # Safety
/// The 'AbiProtocol' protocol must only contain valid data.
pub unsafe fn abi_entry<T: AbiExportableImplementation>(flag: AbiProtocol) {
    match flag {
        AbiProtocol::CreateInstance {
            trait_object_receiver,
            error_receiver,
            error_callback,
        } => {
            let result = catch_unwind(|| {
                let obj: Box<T::AbiInterface> = T::new();
                let raw = Box::into_raw(obj);
                assert_eq!(std::mem::size_of::<*mut T::AbiInterface>(), 16);
                assert_eq!(std::mem::size_of::<TraitObject>(), 16);

                let mut trait_object = TraitObject::zero();

                unsafe {
                    ptr::copy(
                        &raw as *const *mut T::AbiInterface,
                        &mut trait_object as *mut TraitObject as *mut *mut T::AbiInterface,
                        1,
                    )
                };

                unsafe {
                    *trait_object_receiver = trait_object;
                }
            });
            match result {
                Ok(_) => {}
                Err(err) => {
                    let msg: &str;
                    let temp;
                    if let Some(err) = err.downcast_ref::<&str>() {
                        msg = err;
                    } else {
                        temp = format!("{:?}", err);
                        msg = &temp;
                    }
                    let err = AbiErrorMsg {
                        error_msg_utf8: msg.as_ptr(),
                        len: msg.len(),
                    };
                    error_callback(error_receiver, &err as *const _)
                }
            }
        }
        flag => {
            abi_entry_light::<T::AbiInterface>(flag);
        }
    }
}

/// Verify compatibility with old versions.
///
/// If files representing the given AbiExportable definition is not already present,
/// create one file per supported version, with the definition of the ABI.
/// If files are present, verify that the definition is the same as that in the files.
///
/// This allows us to detect if the data structure as we've declared it is modified
/// in a non-backward compatible way.
///
/// 'path' is a path where files defining the Abi schema are stored. These files
/// should be checked in to version control.
pub fn verify_compatiblity<T: AbiExportable + ?Sized>(path: &str) -> Result<(), SavefileError> {
    std::fs::create_dir_all(path)?;
    for version in 0..=T::get_latest_version() {
        let def = T::get_definition(version);
        let schema_file_name = Path::join(Path::new(path), format!("savefile_{}_{}.schema", def.name, version));
        if std::fs::metadata(&schema_file_name).is_ok() {
            let previous_schema = load_file_noschema(&schema_file_name, 1)?;

            def.verify_backward_compatible(version, &previous_schema, false)?;
        } else {
            save_file_noschema(&schema_file_name, 1, &def)?;
        }
    }
    Ok(())
}

#[doc(hidden)]
pub struct AbiWaker {
    #[doc(hidden)]
    waker: Box<dyn Fn() + Send + Sync>,
}
impl AbiWaker {
    pub fn new(waker: Box<dyn Fn() + Send + Sync>) -> Self {
        Self { waker }
    }
}
impl Wake for AbiWaker {
    fn wake(self: Arc<Self>) {
        (self.waker)();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        (self.waker)();
    }
}
