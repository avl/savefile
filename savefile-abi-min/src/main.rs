use savefile_abi::AbiConnection;
use savefile_abi_min_lib::{AdderCallback, AdderInterface, MyStuff};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

struct MyCallback {
    value: Arc<Mutex<u32>>,
}
impl AdderCallback for MyCallback {
    fn set(&self, value: u32) {
        *self.value.lock().unwrap() = value;
    }

    fn get(&self) -> u32 {
        *self.value.lock().unwrap()
    }
}

impl Drop for MyCallback {
    fn drop(&mut self) {
        println!("Dropping AdderCallback");
    }
}

#[inline(never)]
pub fn call_add(adder: &AbiConnection<dyn AdderInterface>, a: u32, b: u32) -> u32 {
    adder.add_simple(a, b)
}
#[no_mangle]
pub extern "C" fn call_do_nothing(adder: &AbiConnection<dyn AdderInterface>) {
    adder.do_nothing();
}

fn main() {
    let connection = AbiConnection::<dyn AdderInterface>::load_shared_library(
        "../target/debug/libsavefile_abi_min_lib_impl.so", //Change this to the proper path on your machine
    )
    .unwrap();

    let res = connection.add(1, &2, &Box::new(MyStuff { x: 43, y: [0; 10000] }));
    assert_eq!(res, 1 + 2 + 43);
    println!("Result: {}", res);
    let my_cb = Box::new(MyCallback {
        value: Arc::new(Mutex::new(32)),
    });

    let my_arc = my_cb.value.clone();
    println!("Before .sub");
    let res2 = connection.sub(4, 1, my_cb);
    assert_eq!(res2, 3);
    println!("Result2: {} {:?}", res2, my_arc.lock().unwrap().deref());

    assert_eq!(call_add(&connection, 1, 2), 3);
}
