use savefile_abi::{AbiConnection};
use savefile_abi_min_lib::AdderInterface;


fn main() {

    let connection = unsafe { AbiConnection::<dyn AdderInterface>::load_shared_library("libsavefile_abi_min_lib_impl.so").unwrap() };

    let res = connection.add(1,2);
    assert_eq!(res,3);
    println!("Result: {}", res);
}
