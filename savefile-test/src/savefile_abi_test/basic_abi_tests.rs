use std::cell::{Cell, UnsafeCell};
use std::io::Cursor;

#[cfg(feature="nightly")]
use test::Bencher;

use savefile::{AbiRemoved, Deserializer, Removed, ValueConstructor, Serialize, Serializer};
use savefile_abi::{abi_entry, AbiConnection, AbiExportable, AbiExportableImplementation};

#[savefile_abi_exportable(version=0)]
pub trait CallbackInterface {
    fn set(&mut self, x: u32);
    fn get(&self) -> u32;
}

#[savefile_abi_exportable(version=0)]
pub trait TestInterface {
    fn add(&self, x: u32, y: String) -> u32;
    fn call_callback(&mut self, callback: &mut dyn CallbackInterface);
    fn do_nothing(&self);
    fn arrays_add(&self, a: &[u32], b: &[u32]) -> Vec<u32>;

    fn do_mut_nothing(&mut self);

    fn zero_sized_arg(&self, zero: ());
    fn simple_add(&self, a: u32, b: u32) -> u32;
}

#[derive(Default)]
pub struct TestInterfaceImpl {

}

pub struct CallbackImpl {
    x: u32,
}

impl CallbackInterface for CallbackImpl {
    fn set(&mut self, x: u32) {
        self.x=x;
    }

    fn get(&self) -> u32 {
        self.x
    }
}

impl TestInterface for TestInterfaceImpl {
    fn add(&self, x: u32, y: String) -> u32 {
        x + y.parse::<u32>().unwrap()
    }

    fn call_callback(&mut self, callback: &mut dyn CallbackInterface) {

        callback.set(42);
    }
    fn arrays_add(&self, a: &[u32], b: &[u32]) -> Vec<u32> {
        let mut ret = Vec::new();
        for (a0,b0) in a.iter().copied().zip(b.iter().copied())
        {
            ret.push(a0 + b0);
        }

        ret
    }
    fn do_nothing(&self) {

    }
    fn do_mut_nothing(&mut self) {

    }

    fn zero_sized_arg(&self, _zero: ()) {

    }

    fn simple_add(&self, a: u32, b: u32) -> u32 {
        a+b
    }
}

savefile_abi_export!(TestInterfaceImpl, TestInterface);

#[test]
fn test_basic_call_abi() {

    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl{});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let mut callback = CallbackImpl {
        x: 43
    };
    conn.call_callback(&mut callback);

    assert_eq!(callback.x, 42);
}
#[test]
fn test_slices() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl{});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let t = conn.arrays_add(&[1,2,3],&[1,2,3]);
    assert_eq!(t, vec![2,4,6]);
}
#[test]
fn test_zero_sized_arg() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl{});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    conn.zero_sized_arg( () );
}
#[test]
fn test_big_slices() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl{});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let a = vec![1u32;10000];
    let b = vec![1u32;10000];

    let t = conn.arrays_add(&a,&b);
    assert_eq!(t.len(), 10000);
    for x in t {
        assert_eq!(x, 2);
    }
}

struct FortyTwoConstructor {

}
impl ValueConstructor<u32> for FortyTwoConstructor {
    fn make_value() -> u32 {
        42
    }
}

#[test]
fn test_abi_removed() {
    let removed:AbiRemoved<u32> = AbiRemoved::new();
    let mut data = Vec::new();
    Serializer::bare_serialize(&mut data, 0, &removed).unwrap();

    let roundtripped: u32 = Deserializer::bare_deserialize(&mut Cursor::new(&data),0).unwrap();
    assert_eq!(roundtripped, 0);
}
#[test]
fn test_abi_removed_with_custom_default() {
    let removed:AbiRemoved<u32, FortyTwoConstructor> = AbiRemoved::<u32, FortyTwoConstructor>::new();
    let mut data = Vec::new();
    Serializer::bare_serialize(&mut data, 0, &removed).unwrap();

    let roundtripped: u32 = Deserializer::bare_deserialize(&mut Cursor::new(&data),0).unwrap();
    assert_eq!(roundtripped, 42);
}


#[cfg(feature="nightly")]
#[bench]
fn bench_simple_add(b: &mut Bencher) {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl{});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    b.iter(move || {
        conn.simple_add(std::hint::black_box(1),std::hint::black_box(2))
    })
}