# Async support in Savefile-abi

[Savefile-abi](https://crates.io/crates/savefile-abi) is a rust crate that allows exposing traits with a 
stable ABI from shared libraries in rust.

Savefile-abi recently gained support for async methods.

This means that you can now define this trait, for example:


```rust 


#[async_trait]
#[savefile_abi_exportable(version = 0)]
pub trait MyDatabaseInterface {
    
    async fn get(&self, key: String) -> String;
    async fn set(&mut self, key: String, value: String);
}


```

You can then create an implementation, and compile it to a freestanding shared library, as a plugin:

```rust

struct MyDatabaseImpl {
    // impl
}

#[async_trait]
impl MyDatabaseInterface for MyDatabaseImpl {
    async fn get(&self, key: String) -> String {
        // call, and await, async methods here 
        "placeholder".to_string()
    }
    async fn set(&mut self, key: String, value: String) {
        // implementation
    }
    
}

savefile_abi_export!(MyDatabaseInterface, MyDatabaseImpl);

```

It is possible to have many such plugins, implementing a particular interface (such as `MyDatabaseInterface` here),
and load these plugins at runtime, without requiring all the plugins to be compiled with the same rust-version 
as the main application.


## Background, what is async good for

(Skip this section if you're already familiar with async)

There are many cases where one might want to use async. Let's look at an example.

Let's say we have a program that calculates the weight of a set of goods, using some logic:

```rust

fn calculate_weight(items: &[Item]) -> u32 {

    let container = get_suitable_container(items.len());

    let mut sum = container.weight;
    for item in items {
        sum += get_item_weight(item);
    }

    sum
}
```

Now, let's say our program needs to do hundreds of thousands of these calculations every second, and let's assume
that the two get_-functions need to do database access and actually have some significant latency. Maybe each execution
of 'calculate_weight' takes 1 second because of this. To process 100000 items per second, we'd then need 100000 threads.

However, the cost of spawning 100000 threads is significant. Instead, we can use async:

```rust

async fn calculate_weight(items: &[Item]) -> u32 {

    let container = get_suitable_container(items.len()).await;

    let mut sum = container.weight;
    for item in items {
        sum += get_item_weight(item).await;
    }

    sum
}
```

Now, using an async runtime such as [tokio](https://tokio.rs/) , many 'calculate_weight'-calculations can
occur simultaneously on the same thread. Under the hood, the above method behaves much as if it was implemented
like this:

```rust
fn calculate_weight(items: &[Item]) -> impl Future<Output=u32> {
    async {
        let container = get_suitable_container(items.len()).await;
        
        let mut sum = container.weight;
        for item in items {
            sum += get_item_weight(item).await;
        }
    }
    sum
}
```

The async keyword constructs a future based on the code block. A future is an object that can be polled, that will
eventually produce a value. Let's look at how the future trait is defined:

```rust
pub trait Future {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```
An async-block such as the one in `calculate_weight` above, implements this trait.

The idea is that the code within the async-block in 'calculate_weight' is converted, by the rust compiler,  into a 
state machine, that will  first call `get_suitable_container` to obtain a future, poll that future until it gets the 
container, then enter the loop and do the same with each future produced by `get_item_weight`. Note that this state 
machine could be  written manually, but it would be much less ergonomic than using an `async` code block.

Async blocks are allowed to contain references to variables inside the async block. This means that the 
future cannot be moved, a fact which is represented using the `Pin<&mut Self>` type for the self-argument to `poll`.

There are of course many more details. See the [Async book](https://rust-lang.github.io/async-book/01_getting_started/04_async_await_primer.html)
for more information.

## Implementing async support in Savefile-abi

My first gut reaction was  that this was going to be very hard, borderline impossible, and difficult to get right. 
As it turns out, it was  actually quite easy, and not even that scary. This blog post describes the effort of adding 
async-support to Savefile-abi.

One thing that needs to be said immediately, up front, is that Savefile-abi only supports a subset of all possible 
trait objects. This is true for savefile abi in general, not only for the async-support. In principle, Savefile-abi is
a bit of a "hack" (though (hopefully), a useful one). Some effort has been made to give detailed and developer-friendly
error messages whenever one of the limitations are hit.

One of the biggest limitations for async-support is that Savefile-abi does not support references for arguments
in async methods. All arguments have to be owned. More about this further down.

The basic way that Savefile-abi works is that implementations of the trait `AbiExportable` are generated for each
trait that is to be exported across shared library boundaries. This trait exposes methods to return the signatures
of all methods in the exported trait, as well as a extern "C"-function that allows calling trait methods. The actual data
is sent as plain bytes in a savefile-abi proprietary binary format, in a forward- and backward-compatible manner.

Savefile-abi already supports boxed dyn trait objects in return position. So the fundamental implementation strategy for
supporting async is to make it possible for Savefile-abi to support returning `Box<Future<Output=?>>`.  One approach 
might be for the savefile-abi crate to simply implement `AbiExportable` for `Box<Future<Output=T>>`, for all T. 
However, this  runs into problems, because the underlying savefile-abi mechanism does not support generics. It is also
not obvious what the trait bounds for T would be.

Instead, the chosen mechanism is to make the `savefile-derive` crate generate new traits, with a Savefile-abi 
compatible signature, and expose these over the savefile abi instead of the standard Future trait. Here's an example of 
such a wrapper for futures producing values of type u32:

```rust
pub trait FutureWrapper {
    fn abi_poll(self: Pin<&mut Self>, waker: Box<dyn Fn()+Send+Sync>) -> Option<u32>;
}

```

The above wrapper is then implemented (using a unique private name) for `Pin<Box<Future<Output=u32>>>`.  A custom implementation
of [Wake](https://doc.rust-lang.org/std/task/trait.Wake.html) is created, and used to create a 
[Waker](https://doc.rust-lang.org/std/task/struct.Waker.html) and 
[Context](https://doc.rust-lang.org/std/task/struct.Context.html) that can  be passed to the `poll` method of `Future`.  
This custom waker ten simply calls the Boxed dyn Fn `waker` of `abi_poll`.

The challenge here is that Savefile-abi did not previously support `Pin<&mut Self>` for self. Adding this
support was relatively straightforward, since, under the hood, `Pin<&mut Self>` is still just a regular self-pointer.

The next challenge is lifetimes. Savefile abi can never support lifetimes in return position. The reason for this
is that savefile falls back on serialization, if memory layout of data types changes between different 
versions of an interface. This means that only owned values can be supported, since it's impossible to create 
deserialized  values with arbitrary lifetimes.

However, futures can normally capture argument lifetimes, especially that of 'self'. Supporting reference arguments
in functions returning futures turns out to be hard, since the references may have to be serialized before 
sending to the callee, which means that the return value can't be allowed capture them. However, we can support
futures that capture 'self', since self is never serialized in Savefile-abi. Expressing that a future
captures 'self' is often done using lifetime-annotations, something Savefile-abi previously did not support.

That said, this was always an arbitrary and unnecessary limitation. Even if `self` is written as just `&self`,
it still has a lifetime, even if that lifetime isn't given a name. There's no reason why Savefile shouldn't
support a method like this:

```rust
    fn some_function<'a>(&'a self) -> u32; 
```

Previously, there was also no reason _why_ such a method should be supported, since the annotation
doesn't add anything. However, users of savefile abi will probably want to use the `#[async_trait]`-macro
(see [async-trait](https://docs.rs/async-trait/latest/async_trait/)).

This macro does add lifetime annotations, which can look something like this:

```rust
fn set<'life0, 'async_trait>(
        &'life0 self,
        value: u32,
    ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait; 
```
The above syntax describes that the returned future is (potentially) capturing `self`.

To be able to use Savefile abi with the 'async_trait'-macro, the above must be supported. However, most usages
of lifetimes in Savefile abi don't make sense, since savefile can always fall back on serialization.

As of version 0.18.4, savefile-abi uses heuristics to detect usage of `#[async_trait]`, and allows lifetime annotations
_only_ if they appear to follow the pattern used by async_trait. This limitation could be relaxed. The main
challenge is making sure that all the code that savefile-derive generates is compatible with the lifetime annotations,
and also making sure that adding such annotations can't be used to somehow cause unsoundness. Basically, as long
as we don't allow references in return position, nothing that can be expressed using lifetime annotations or where-clauses 
should cause a problem, so this is definitely something to do in future versions of savefile-abi. 


## Performance

As described above, savefile-abi relies on boxed futures. This means that each call to an async-function requires
memory allocation. Savefile-abi is thus definitely not free. A simple benchmark of a minimal async-invocation,
gives an overhead of ca 200 ns (on an AMD 7950X CPU). This is much, much more expensive than a regular function 
call, but typically cheaper than a gRPC-call or other IPC.

It is probably possible to optimize the performance at least slightly, by trying to reduce the need for memory 
allocation. Right now the Waker requires several memory allocations, something that could conceivably be alleviated.



























