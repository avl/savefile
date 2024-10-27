use async_trait::async_trait;
use savefile_abi::{AbiConnection, AbiWaker};
use std::future::Future;
use std::hint::black_box;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
#[cfg(feature = "nightly")]
use test::Bencher;
use tokio::pin;

#[async_trait]
#[savefile_abi_exportable(version = 0)]
pub trait SimpleAsyncInterface {
    async fn add_async(&mut self, x: u32, y: u32) -> u32;
    async fn add_async2(&self, x: u32, y: u32) -> u32;
    async fn internal_inc(&mut self, x: u32) -> u32;

}

#[savefile_abi_exportable(version = 0)]
pub trait BoxedAsyncInterface {
    fn add_async(&mut self, x: u32, y: u32) -> Pin<Box<dyn Future<Output=String>>>;

}

#[derive(Default)]
struct SimpleImpl {
    internal: u32
}

impl BoxedAsyncInterface for SimpleImpl {
    fn add_async(&mut self, x: u32, y: u32) -> Pin<Box<dyn Future<Output=String>>> {
        Box::pin(
            async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                format!("{}",x+y)
            }
        )
    }
}

#[async_trait]
impl SimpleAsyncInterface for SimpleImpl {
    async fn add_async(&mut self, x: u32, y: u32) -> u32 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        x + y
    }

    async fn add_async2(&self, x: u32, y: u32) -> u32 {
        x + y
    }
    async fn internal_inc(&mut self, x: u32) -> u32 {
        tokio::time::sleep(Duration::from_millis(1)).await;
        self.internal += x;
        self.internal
    }
}

#[tokio::test]
async fn abi_test_async() {
    let boxed: Box<dyn SimpleAsyncInterface> = Box::new(SimpleImpl::default());
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let mut acc = 0;
    for i in 0..10 {
        assert_eq!(acc + i, conn.add_async2(acc, i).await);
        acc = conn.add_async(acc, i).await;
    }

    assert_eq!(acc, 45);
    assert_eq!(conn.internal_inc(10).await, 10);
    assert_eq!(conn.internal_inc(10).await, 20);

}

#[tokio::test]
async fn abi_test_boxed_async() {
    let boxed: Box<dyn BoxedAsyncInterface> = Box::new(SimpleImpl::default());
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    assert_eq!(conn.add_async(1,2).await, "3");
}


#[cfg(feature = "nightly")]
#[cfg(not(miri))]
#[bench]
fn bench_simple_async_call(b: &mut Bencher) {
    let boxed: Box<dyn SimpleAsyncInterface> = Box::new(SimpleImpl::default());
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    b.iter(|| {
        let waker = Waker::from(Arc::new(AbiWaker::new(Box::new(|| {}))));
        let mut context = Context::from_waker(&waker);
        let x = conn.add_async2(1, 2);
        pin!(x);
        match x.poll(&mut context) {
            Poll::Ready(sum) => black_box(sum),
            Poll::Pending => {
                unreachable!()
            }
        }
    })
}
