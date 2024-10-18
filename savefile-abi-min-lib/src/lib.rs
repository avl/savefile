use savefile_derive::savefile_abi_exportable;
use savefile_derive::Savefile;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::{pin, Pin};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;
use savefile_abi::AbiWaker;

#[savefile_abi_exportable(version = 0)]
pub trait AdderInterface {
    fn boxed_future(&self) -> Box<dyn Future<Output=u32> + Unpin>;
   // fn pinned_self(&self, arg: u32) -> Box<dyn Future<Output = u32>>;
}
/*
pub trait Future {
    /// The type of value produced on completion.
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[lang = "future_output"]
    type Output;
   #[lang = "poll"]
    #[stable(feature = "futures_api", since = "1.36.0")]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
*/


/*
#[savefile_abi_exportable(version = 0)]
pub trait FutureWrapper {
    fn abi_poll(self: Pin<&mut Self>, waker: Box<dyn FnMut()+Send+Sync>) -> Option<u32>;
}
impl Future for Box<dyn FutureWrapper> {
    type Output = u32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut waker = Some(cx.waker().clone());

        match unsafe { self.map_unchecked_mut(|s|&mut **s)}.abi_poll(Box::new(move ||{waker.take().map(|x|x.wake());})) {
            Some(temp) => {
                println!("Poll::ready!");
                Poll::Ready(temp)
            }
            None => {
                println!("Poll::pending!");
                Poll::Pending
            }
        }
    }
}
impl FutureWrapper for Box<dyn Future<Output = u32>> {
    fn abi_poll(self: Pin<&mut Self>, waker: Box<dyn FnMut()+Send+Sync>) -> Option<u32> {
        println!("abi_poll");
        let waker = Waker::from(Arc::new(AbiWaker {
            waker: waker.into()
        }));
        let mut context = Context::from_waker(&waker);

        println!("delegating");
        match unsafe { self.map_unchecked_mut(|s|&mut **s)}.poll(&mut context) {
            Poll::Ready(t) => {
                println!("Done!");
                Some(t)
            }
            Poll::Pending => {
                println!("Pending!");
                None
            }
        }
    }
}
*/

/*

#[derive(Savefile)]
pub struct MyStuff {
    pub x: u64,
    pub y: [u64; 10_000],
}

#[savefile_abi_exportable(version = 0)]
pub trait AdderCallback {
    fn set(&self, value: u32);
    fn get(&self) -> u32;
}

#[savefile_abi_exportable(version = 0)]
pub trait AdderInterface {
    fn add(&self, x: u32, y: &u32, z: &MyStuff) -> u32;
    fn sub(&self, x: u32, y: u32, cb: Box<dyn AdderCallback>) -> u32;
    fn add_simple(&self, x: u32, y: u32) -> u32;
    fn do_nothing(&self);
}
impl Debug for dyn AdderInterface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AdderInterface")
    }
}
*/

#[tokio::test]
async fn test_future() {

    fn subfunc() -> Pin<Box<dyn Future<Output=u32>>> {

        let timer : Box<dyn Future<Output=u32>> = Box::new(async {
            tokio::time::timeout(Duration::from_secs(2), std::future::pending::<u32>()).await.unwrap_err();
            42
        });

        let future_wrapper: Box<dyn FutureWrapper> = Box::new(timer);
        let s = future_wrapper;

        Box::pin(s)
    }

    let fut = subfunc().await;


}