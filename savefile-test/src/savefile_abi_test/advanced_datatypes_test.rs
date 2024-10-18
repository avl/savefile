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
pub trait AdvancedTestInterface : Send{
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String>;
    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String>;

    fn return_trait_object(&self) -> Box<dyn SimpleInterface>;
    fn test_slices(&mut self, slice: &[u32]) -> u32;

    fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32>;
    fn return_boxed_closure2(&self) -> Box<dyn Fn()>;
    fn many_callbacks(&mut self, x: &mut dyn FnMut(&dyn Fn(&dyn Fn() -> u32) -> u32) -> u32) -> u32;

    fn buf_callback(&mut self, cb: Box<dyn Fn(&[u8], String) + Send + Sync>);
    fn return_boxed_closure_result(&self, fail: bool) -> Result<Box<dyn Fn() -> u32>,()>;
    fn owned_boxed_closure_param(&self, owned: Box<dyn Fn()->u32>);


    fn pinned_self(self: Pin<&mut Self>, arg: u32) -> u32;
    fn boxed_future(&self) -> Box<dyn Future<Output=u32> + Unpin>;
}
/*
pub trait Future {
    /// The type of value produced on completion.
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[lang = "future_output"]
    type Output;
   /// Attempts to resolve the future to a final value, registering
    /// the current task for wakeup if the value is not yet available.
    ///
    /// # Return value
    ///
    /// This function returns:
    ///
    /// - [`Poll::Pending`] if the future is not ready yet
    /// - [`Poll::Ready(val)`] with the result `val` of this future if it
    ///   finished successfully.
    ///
    /// Once a future has finished, clients should not `poll` it again.
    ///
    /// When a future is not ready yet, `poll` returns `Poll::Pending` and
    /// stores a clone of the [`Waker`] copied from the current [`Context`].
    /// This [`Waker`] is then woken once the future can make progress.
    /// For example, a future waiting for a socket to become
    /// readable would call `.clone()` on the [`Waker`] and store it.
    /// When a signal arrives elsewhere indicating that the socket is readable,
    /// [`Waker::wake`] is called and the socket future's task is awoken.
    /// Once a task has been woken up, it should attempt to `poll` the future
    /// again, which may or may not produce a final value.
    ///
    /// Note that on multiple calls to `poll`, only the [`Waker`] from the
    /// [`Context`] passed to the most recent call should be scheduled to
    /// receive a wakeup.
    ///
    /// # Runtime characteristics
    ///
    /// Futures alone are *inert*; they must be *actively* `poll`ed to make
    /// progress, meaning that each time the current task is woken up, it should
    /// actively re-`poll` pending futures that it still has an interest in.
    ///
    /// The `poll` function is not called repeatedly in a tight loop -- instead,
    /// it should only be called when the future indicates that it is ready to
    /// make progress (by calling `wake()`). If you're familiar with the
    /// `poll(2)` or `select(2)` syscalls on Unix it's worth noting that futures
    /// typically do *not* suffer the same problems of "all wakeups must poll
    /// all events"; they are more like `epoll(4)`.
    ///
    /// An implementation of `poll` should strive to return quickly, and should
    /// not block. Returning quickly prevents unnecessarily clogging up
    /// threads or event loops. If it is known ahead of time that a call to
    /// `poll` may end up taking a while, the work should be offloaded to a
    /// thread pool (or something similar) to ensure that `poll` can return
    /// quickly.
    ///
    /// # Panics
    ///
    /// Once a future has completed (returned `Ready` from `poll`), calling its
    /// `poll` method again may panic, block forever, or cause other kinds of
    /// problems; the `Future` trait places no requirements on the effects of
    /// such a call. However, as the `poll` method is not marked `unsafe`,
    /// Rust's usual rules apply: calls must never cause undefined behavior
    /// (memory corruption, incorrect use of `unsafe` functions, or the like),
    /// regardless of the future's state.
    ///
    /// [`Poll::Ready(val)`]: Poll::Ready
    /// [`Waker`]: crate::task::Waker
    /// [`Waker::wake`]: crate::task::Waker::wake
    #[lang = "poll"]
    #[stable(feature = "futures_api", since = "1.36.0")]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
*/
/*
pub trait __0_future_ {
    fn poll(&mut self) -> ();
}
struct __0_future_wrapper<'a> {
    func: Box<(dyn for<'x> Fn() + 'a)>,
}
impl<'a> __0_owning_ for __0_owning_wrapper<'a> {
    fn docall(&self) -> () {
        unsafe { (&*self.func)() }
    }
}
unsafe extern "C" fn abi_entry_light_AdderInterface(flag: AbiProtocol) {
    unsafe {
        abi_entry_light::<dyn AdderInterface>(flag);
    }
}
*/

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
        cb(&[1,2,3], "hello".to_string())
    }
    fn return_boxed_closure_result(&self, fail: bool) -> Result<Box<dyn Fn() -> u32>,()> {
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
    fn boxed_future(&self) -> Box<dyn Future<Output=u32> + Unpin> {
        Box::new(Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
            42
        }))
    }
}

struct TestUser(Box<dyn AdvancedTestInterface + 'static>);

pub trait DummyTrait2 : Send {

}

impl DummyTrait2 for TestUser {

}
fn require_send<T:Send>(_t: T) {

}
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
    let Err(()) = ret else {panic!("Expected Err")};


}

#[test]
fn abi_test_buf_callback() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let buf = Arc::new(Mutex::new(None));
    let bufclone = Arc::clone(&buf);
    conn.buf_callback(Box::new(move|argbuf, _s|{
        *bufclone.lock().unwrap() = Some(argbuf.to_vec());
    }));
    let mut guard = buf.lock().unwrap();
    let vec = guard.take().unwrap();
    assert_eq!(vec, [1,2,3]);

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
    //let timeout = fut.await;
    println!("After timeout");
}

#[test]
fn test_boxed_trait_object_in_arg_position() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    conn.owned_boxed_closure_param(Box::new(||42));
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
