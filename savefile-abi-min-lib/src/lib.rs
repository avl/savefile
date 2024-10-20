use savefile_derive::savefile_abi_exportable;
use savefile_derive::Savefile;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;


/*
#[async_trait::async_trait]
#[savefile_abi_exportable(version = 0)]
pub trait AdderCallback {
    async fn set(&self, value: u32) -> u32;
}
*/

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

    fn async_add(&self, x: u32, y: u32) -> Pin<Box<dyn Future<Output = u32>>>;
}
impl Debug for dyn AdderInterface {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AdderInterface")
    }
}


#[test]
fn test(){

}
