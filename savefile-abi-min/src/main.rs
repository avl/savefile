use savefile::WithSchema;
use savefile_abi::{AbiConnection};
use savefile_abi_min_lib::{AdderInterface, MyStuff};




fn main() {

    let connection = unsafe { AbiConnection::<dyn AdderInterface>::load_shared_library("libsavefile_abi_min_lib_impl.so").unwrap() };

    println!("Info: {:#?}", MyStuff::schema(0));
    println!("Conn: {:#?}", connection);
    let res = connection.add(1,&2, &*Box::new(MyStuff {
        x: 43,
        y: [0;10000],
    }));
    assert_eq!(res,1+2+43);
    println!("Result: {}", res);
    let res2 = connection.sub(2,1);
    assert_eq!(res2, 1);
    println!("Result2: {}", res2);
}
