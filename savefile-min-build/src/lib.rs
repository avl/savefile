extern crate savefile_abi;
extern crate savefile_derive;

use savefile_abi::AbiConnection;
use savefile_derive::savefile_abi_exportable;


#[savefile_abi_exportable(version = 0)]
pub trait ExampleTrait {
    fn test_slices(&mut self, slice: &[u32]) -> u32 {
        slice.iter().copied().sum()
    }
}

impl ExampleTrait for () {

}

#[test]
fn dummy_test() {
    let boxed: Box<dyn ExampleTrait> = Box::new(());
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    assert!( conn.get_arg_passable_by_ref("test_slices", 0) );
    //conn.test_slices(&[1,2,3,4]);
}
