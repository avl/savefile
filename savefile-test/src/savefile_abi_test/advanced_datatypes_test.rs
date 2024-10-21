use savefile_abi::AbiConnection;
use savefile_abi::AbiExportable;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

#[savefile_abi_exportable(version = 0)]
pub trait SimpleInterface {
    fn do_call(&self, x: u32) -> u32;
}
#[savefile_abi_exportable(version = 0)]
pub trait AdvancedTestInterface: Send {
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String>;
    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String>;

    fn return_trait_object(&self) -> Box<dyn SimpleInterface>;
    fn test_slices(&mut self, slice: &[u32]) -> u32;

    fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32>;
    fn return_boxed_closure2(&self) -> Box<dyn Fn()>;
    fn many_callbacks(&mut self, x: &mut dyn FnMut(&dyn Fn(&dyn Fn() -> u32) -> u32) -> u32) -> u32;

    fn buf_callback(&mut self, cb: Box<dyn Fn(&[u8], String) + Send + Sync>);
    fn return_boxed_closure_result(&self, fail: bool) -> Result<Box<dyn Fn() -> u32>, ()>;
    fn owned_boxed_closure_param(&self, owned: Box<dyn Fn() -> u32>);

    fn pinned_self(self: Pin<&mut Self>, arg: u32) -> u32;
    fn boxed_future(&self) -> Pin<Box<dyn Future<Output = u32>>>;
}

struct SimpleImpl;

impl Drop for SimpleImpl {
    fn drop(&mut self) {
        println!("Dropping impl")
    }
}
impl SimpleInterface for SimpleImpl {
    fn do_call(&self, x: u32) -> u32 {
        println!("do_call running");
        x
    }
}
struct AdvancedTestInterfaceImpl {}

impl AdvancedTestInterface for AdvancedTestInterfaceImpl {
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String> {
        x
    }

    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String> {
        x.clone()
    }

    fn return_trait_object(&self) -> Box<dyn SimpleInterface> {
        Box::new(SimpleImpl)
    }

    fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32> {
        Box::new(|| 42)
    }
    fn return_boxed_closure2(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    fn test_slices(&mut self, slice: &[u32]) -> u32 {
        slice.iter().copied().sum()
    }

    fn many_callbacks(&mut self, x: &mut dyn FnMut(&dyn Fn(&dyn Fn() -> u32) -> u32) -> u32) -> u32 {
        x(&|y| y())
    }

    fn buf_callback(&mut self, cb: Box<dyn Fn(&[u8], String) + Send + Sync>) {
        cb(&[1, 2, 3], "hello".to_string())
    }
    fn return_boxed_closure_result(&self, fail: bool) -> Result<Box<dyn Fn() -> u32>, ()> {
        if fail {
            Err(())
        } else {
            Ok(Box::new(|| 42))
        }
    }

    fn owned_boxed_closure_param(&self, owned: Box<dyn Fn() -> u32>) {
        assert_eq!(owned(), 42);
    }
    fn pinned_self(self: Pin<&mut Self>, arg: u32) -> u32 {
        arg
    }
    fn boxed_future(&self) -> Pin<Box<dyn Future<Output = u32>>> {
        Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            42
        })
    }
}

struct TestUser(Box<dyn AdvancedTestInterface + 'static>);

pub trait DummyTrait2: Send {}

impl DummyTrait2 for TestUser {}
fn require_send<T: Send>(_t: T) {}
#[test]
fn abi_test_buf_send() {
    let boxed: Box<dyn AdvancedTestInterface + Send + Sync> = Box::new(AdvancedTestInterfaceImpl {});
    require_send(boxed);
}

#[test]
fn test_trait_object_in_return_position() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let ret = conn.return_boxed_closure_result(false);
    assert_eq!(ret.unwrap()(), 42);
    let ret = conn.return_boxed_closure_result(true);
    let Err(()) = ret else { panic!("Expected Err") };
}

#[test]
fn abi_test_buf_callback() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let buf = Arc::new(Mutex::new(None));
    let bufclone = Arc::clone(&buf);
    conn.buf_callback(Box::new(move |argbuf, _s| {
        *bufclone.lock().unwrap() = Some(argbuf.to_vec());
    }));
    let mut guard = buf.lock().unwrap();
    let vec = guard.take().unwrap();
    assert_eq!(vec, [1, 2, 3]);
}
#[test]
fn abi_test_slice() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    assert!(conn.get_arg_passable_by_ref("test_slices", 0));
    assert_eq!(conn.test_slices(&[1, 2, 3, 4]), 10);
}

#[test]
fn test_result_trait_object_in_return_position() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let ret = conn.return_trait_object();
    assert_eq!(ret.do_call(42), 42);
    assert_eq!(ret.do_call(42), 42);
}

#[tokio::test]
async fn test_boxed_future() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    println!("Before timeout");

    let fut = conn.boxed_future();

    fut.await;
    println!("After timeout");
}

#[test]
fn test_boxed_trait_object_in_arg_position() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    conn.owned_boxed_closure_param(Box::new(|| 42));
}
#[test]
fn test_return_boxed_closure() {
    let closure;
    let closure2;
    {
        let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
        let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

        closure = conn.return_boxed_closure();
        closure2 = conn.return_boxed_closure2();
        assert_eq!(closure(), 42);
    }
    assert_eq!(closure(), 42);
    closure2();
}

#[test]
fn test_call_many_callbacks() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    assert_eq!(
        conn.many_callbacks(&mut |x| {
            x(&|| {
                println!("In the inner sanctum!");
                42
            })
        }),
        42
    );
}
#[test]
fn test_advanced_abi2() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let mut mymap = HashMap::new();
    mymap.insert("mascot".to_string(), "ferris".to_string());
    mymap.insert("concurrency".to_string(), "fearless".to_string());
    let mymap = conn.roundtrip_hashmap(mymap);

    let mymap2: HashMap<String, String> = conn.clone_hashmap(&mymap);

    assert!(mymap2.contains_key("mascot"));
    assert_eq!(mymap2["mascot"], "ferris");
}
