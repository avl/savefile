use savefile_abi::{AbiConnection};
use interface_crate::{AdderInterface};


fn main() {
    // Load the implementation of `dyn AdderInterface` that was published
    // using the `savefile_abi_export!` above.
    let connection = AbiConnection::<dyn AdderInterface>
            ::load_shared_library("./ImplementationCrate.so").unwrap();

    // The type `AbiConnection::<dyn AdderInterface>` implements
    // the `AdderInterface`-trait, so we can use it to call its methods.
    assert_eq!(connection.add(1, 2), 3);
}


