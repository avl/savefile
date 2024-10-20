use async_trait::async_trait;
use savefile_abi::AbiConnection;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

#[async_trait]
#[savefile_abi_exportable(version = 0)]
pub trait SimpleAsyncInterface {
    async fn add_async(&mut self, x: u32, y: u32) -> u32;
    async fn add_async2(&self, x: u32, y: u32) -> u32;
}

struct SimpleImpl;

#[async_trait]
impl SimpleAsyncInterface for SimpleImpl {
    async fn add_async(&mut self, x: u32, y: u32) -> u32 {
        tokio::time::sleep(Duration::from_millis(1)).await;
        x + y
    }

    async fn add_async2(&self, x: u32, y: u32) -> u32 {
        x + y
    }
}

#[tokio::test]
async fn abi_test_slice() {
    let boxed: Box<dyn SimpleAsyncInterface> = Box::new(SimpleImpl);
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    let mut acc = 0;
    for i in 0..10 {
        assert_eq!(acc + i, conn.add_async2(acc, i).await);
        acc = conn.add_async(acc, i).await;
    }

    assert_eq!(acc, 45);
}
