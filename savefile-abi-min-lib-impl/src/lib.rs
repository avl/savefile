use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use async_std::future;
use async_std::future::timeout;
use async_std::net::UdpSocket;
use savefile_abi_min_lib::{AdderCallback, AdderInterface, MyStuff};
use savefile_derive::savefile_abi_export;

pub struct AdderImplementation {
    _name: String,
}

impl Drop for AdderImplementation {
    fn drop(&mut self) {
        println!("Adder being dropped");
    }
}

impl Default for AdderImplementation {
    fn default() -> Self {
        AdderImplementation {
            _name: "Adderaren Kalle".to_string(),
        }
    }
}

impl AdderInterface for AdderImplementation {
    fn add(&self, x: u32, y: &u32, z: &MyStuff) -> u32 {
        x + y + (z.x as u32)
    }

    fn sub(&self, x: u32, y: u32, cb: Box<dyn AdderCallback>) -> u32 {
        let ret = x.saturating_sub(y);
        cb.set(ret);
        println!("----AFTER cb returned----");
        ret
    }

    fn add_simple(&self, x: u32, y: u32) -> u32 {
        x + y
    }

    fn do_nothing(&self) {}

    fn async_add(&self, x: u32, y: u32) -> Pin<Box<dyn Future<Output=u32>>> {
        Box::pin(async move {
            println!("Begin async-std timeout");
            let udp = UdpSocket::bind(("0.0.0.0", 7777)).await.unwrap();
            udp.send_to(&[42], "127.0.0.1:8888").await.unwrap();
            let _ = timeout(Duration::from_secs(5), future::pending::<()>()).await;
            println!("Async-std timeout resolved");
            x + y
        })
    }
}
savefile_abi_export!(AdderImplementation, AdderInterface);
