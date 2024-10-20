#![allow(unused)]
use std::borrow::Cow;
use std::cell::{Cell, UnsafeCell};
use std::io::Cursor;

#[cfg(feature = "nightly")]
use test::Bencher;

use savefile::{AbiRemoved, Deserializer, Removed, Serialize, Serializer, ValueConstructor};
use savefile_abi::{abi_entry, AbiConnection, AbiExportable, AbiExportableImplementation};

#[savefile_abi_exportable(version = 0)]
pub trait CallbackInterface {
    fn set(&mut self, x: u32);
    fn get(&self) -> u32;
}

#[derive(Savefile)]
pub struct SomeRandomType;

#[savefile_abi_exportable(version = 0)]
pub trait TestInterface {
    fn add(&self, x: u32, y: String) -> u32;
    fn call_callback(&mut self, callback: &mut dyn CallbackInterface);
    fn do_nothing(&self);
    fn do_panic(&self);
    fn arrays_add(&self, a: &[u32], b: &[u32]) -> Vec<u32>;
    fn string_arrays_add(&self, a: &[String], b: &[String]) -> Vec<String>;

    fn do_mut_nothing(&mut self);

    fn deref_u32(&self, x: &u32) -> u32;
    fn count_chars(&self, x: &String) -> usize;
    fn count_chars_str(&self, x: &str) -> usize;

    fn zero_sized_arg(&self, zero: ());
    fn simple_add(&self, a: u32, b: u32) -> u32;

    fn tuple_add1(&self, a: (u32,), b: (u32,)) -> (u32,);
    fn tuple_add2(&self, a: (u32, u32), b: (u32, u32)) -> (u32, u32);
    fn tuple_add3(&self, a: (u32, u32, u32), b: (u32, u32, u32)) -> (u32, u32, u32);

    fn boxes(&self, a: Box<u32>) -> Box<u32>;

    fn test_default_impl(&self) -> String {
        "hello".to_string()
    }

    fn get_static_str(&self) -> &'static str;
    // Test using lots of symbol-names from the derive-macro, to verify
    // there's no crashes
    fn test_macro_hygiene(
        &self,
        context: SomeRandomType,
        schema: SomeRandomType,
        trait_object: SomeRandomType,
        get_schema: SomeRandomType,
        method_number: SomeRandomType,
        effective_version: SomeRandomType,
        new: SomeRandomType,
        result_buffer: SomeRandomType,
        compatibility_mask: SomeRandomType,
        callee_method_number: SomeRandomType,
        info: SomeRandomType,
        serializer: SomeRandomType,
        outcome: SomeRandomType,
        result_receiver: SomeRandomType,
        abi_result_receiver: SomeRandomType,
        resval: SomeRandomType,
        abi_result: SomeRandomType,
        err_str: SomeRandomType,
        ret: SomeRandomType,
        cursor: SomeRandomType,
        deserializer: SomeRandomType,
    ) {
    }
}

#[derive(Default)]
pub struct TestInterfaceImpl {}

pub struct CallbackImpl {
    x: u32,
}

impl CallbackInterface for CallbackImpl {
    fn set(&mut self, x: u32) {
        self.x = x;
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
        for (a0, b0) in a.iter().copied().zip(b.iter().copied()) {
            ret.push(a0 + b0);
        }

        ret
    }
    fn do_nothing(&self) {}
    fn do_panic(&self) {
        panic!("TestInterface was asked to panic")
    }
    fn do_mut_nothing(&mut self) {}

    fn zero_sized_arg(&self, _zero: ()) {}

    fn simple_add(&self, a: u32, b: u32) -> u32 {
        a + b
    }

    fn tuple_add1(&self, a: (u32,), b: (u32,)) -> (u32,) {
        (a.0 + b.0,)
    }

    fn tuple_add2(&self, a: (u32, u32), b: (u32, u32)) -> (u32, u32) {
        (a.0 + b.0, a.1 + b.1)
    }

    fn tuple_add3(&self, a: (u32, u32, u32), b: (u32, u32, u32)) -> (u32, u32, u32) {
        (a.0 + b.0, a.1 + b.1, a.2 + b.2)
    }

    fn boxes(&self, a: Box<u32>) -> Box<u32> {
        a
    }

    fn string_arrays_add(&self, a: &[String], b: &[String]) -> Vec<String> {
        let mut ret = vec![];
        for (a1, b1) in a.iter().zip(b.iter()) {
            ret.push(a1.to_string() + b1);
        }
        ret
    }

    fn count_chars(&self, x: &String) -> usize {
        x.len()
    }
    fn count_chars_str(&self, x: &str) -> usize {
        x.len()
    }

    fn get_static_str(&self) -> &'static str {
        "hello world"
    }

    fn deref_u32(&self, x: &u32) -> u32 {
        *x
    }
}

savefile_abi_export!(TestInterfaceImpl, TestInterface);

#[test]
fn test_basic_call_abi() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let mut callback = CallbackImpl { x: 43 };
    conn.call_callback(&mut callback);

    assert_eq!(callback.x, 42);

    assert_eq!(conn.tuple_add1((1,), (2,)), (3,));
    assert_eq!(conn.tuple_add2((1, 1), (2, 2)), (3, 3));
    assert_eq!(conn.tuple_add3((1, 1, 1), (2, 2, 2)), (3, 3, 3));
    assert_eq!(conn.boxes(Box::new(42u32)), Box::new(42u32));
    assert_eq!(conn.test_default_impl(), "hello");

    assert_eq!(conn.count_chars(&"hejsan".to_string()), 6);
    assert_eq!(conn.count_chars_str("hejsan"), 6);
    assert!(conn.get_arg_passable_by_ref("count_chars", 0));
    assert_eq!(conn.get_static_str(), "hello world");

    assert_eq!(conn.deref_u32(&42), 42);
    assert!(conn.get_arg_passable_by_ref("deref_u32", 0));
}

#[test]
fn test_slices() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let t = conn.arrays_add(&[1, 2, 3], &[1, 2, 3]);
    assert_eq!(t, vec![2, 4, 6]);

    let t = conn.string_arrays_add(&["hello ".to_string()], &["world".to_string()]);
    assert_eq!(t, vec!["hello world"]);
}

#[test]
fn test_zero_sized_arg() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    conn.zero_sized_arg(());
}
#[test]
#[should_panic(expected = "TestInterface was asked to panic")]
fn test_panicking() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    conn.do_panic();
}

#[test]
fn test_big_slices() {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let a = vec![1u32; 10000];
    let b = vec![1u32; 10000];

    let t = conn.arrays_add(&a, &b);
    assert_eq!(t.len(), 10000);
    for x in t {
        assert_eq!(x, 2);
    }
}

struct FortyTwoConstructor {}
impl ValueConstructor<u32> for FortyTwoConstructor {
    fn make_value() -> u32 {
        42
    }
}

#[test]
fn test_abi_removed() {
    let removed: AbiRemoved<u32> = AbiRemoved::new();
    let mut data = Vec::new();
    Serializer::bare_serialize(&mut data, 0, &removed).unwrap();

    let roundtripped: u32 = Deserializer::bare_deserialize(&mut Cursor::new(&data), 0).unwrap();
    assert_eq!(roundtripped, 0);
}
#[test]
fn test_abi_removed_with_custom_default() {
    let removed: AbiRemoved<u32, FortyTwoConstructor> = AbiRemoved::<u32, FortyTwoConstructor>::new();
    let mut data = Vec::new();
    Serializer::bare_serialize(&mut data, 0, &removed).unwrap();

    let roundtripped: u32 = Deserializer::bare_deserialize(&mut Cursor::new(&data), 0).unwrap();
    assert_eq!(roundtripped, 42);
}

#[cfg(feature = "nightly")]
#[cfg(not(miri))]
#[bench]
fn bench_simple_call(b: &mut Bencher) {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    b.iter(move || conn.do_nothing())
}

#[cfg(feature = "nightly")]
#[cfg(not(miri))]
#[bench]
fn bench_simple_add(b: &mut Bencher) {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    b.iter(move || conn.simple_add(std::hint::black_box(1), std::hint::black_box(2)))
}
#[cfg(feature = "nightly")]
#[cfg(not(miri))]
#[bench]
fn bench_count_chars(b: &mut Bencher) {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let mut s = String::new();
    use std::fmt::Write;
    for i in 0..10000 {
        write!(s, "{}", i).unwrap();
    }
    b.iter(move || conn.count_chars(std::hint::black_box(&s)))
}
#[cfg(feature = "nightly")]
#[cfg(not(miri))]
#[bench]
fn bench_count_chars_str(b: &mut Bencher) {
    let boxed: Box<dyn TestInterface> = Box::new(TestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let mut s = String::new();
    use std::fmt::Write;
    for i in 0..10000 {
        write!(s, "{}", i).unwrap();
    }
    b.iter(move || conn.count_chars_str(std::hint::black_box(&s)))
}

#[savefile_abi_exportable(version = 0)]
pub trait CowSmuggler {
    // Specifying &'static is supported. Otherwise, the lifetime
    // becomes artificially short in this case (it becomes that of &self).
    fn smuggle2(&mut self, x: Cow<str>) -> Cow<'static, str>;
    // In this case, the lifetime of Cow is that of &mut self.
    // (Rust lifetime elision rules).
    fn smuggle(&mut self, x: Cow<str>) -> Cow<str>;
}
impl CowSmuggler for () {
    fn smuggle(&mut self, x: Cow<str>) -> Cow<str> {
        (*x).to_owned().into()
    }
    fn smuggle2(&mut self, x: Cow<str>) -> Cow<'static, str> {
        (*x).to_owned().into()
    }
}

#[test]
fn test_cow_smuggler() {
    let boxed: Box<dyn CowSmuggler> = Box::new(());
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    assert_eq!(conn.smuggle("hej".into()), "hej");
    assert_eq!(conn.smuggle("hej".to_string().into()), "hej");

    assert_eq!(conn.smuggle2("hej".into()), "hej");
    assert_eq!(conn.smuggle2("hej".to_string().into()), "hej");

    let static_ret: Cow<'static, str> = conn.smuggle2("hej".into());
    assert_eq!(static_ret, "hej");
}
