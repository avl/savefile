use savefile_abi::{AbiConnection, AbiExportable};
use savefile_abi_test::basic_abi_tests::{TestInterface, TestInterfaceImpl};

#[derive(Savefile,Clone)]
#[repr(C, u8)]
pub enum AbiSimpleEnum {
    Variant1,
    Variant2(u32, String, String, String),
    Variant3(Vec<u8>,Vec<()>)
}

#[savefile_abi_exportable(version=0)]
pub trait SimpleInterfaceWithEnums {
    fn count_arg(&self, x: &AbiSimpleEnum) -> u32;
    fn count_arg_owned(&self, x: AbiSimpleEnum) -> u32;

    fn closure_arg(&self, x: &dyn Fn(&AbiSimpleEnum) -> AbiSimpleEnum) -> AbiSimpleEnum;
}

struct Implementation {

}

impl SimpleInterfaceWithEnums for Implementation {
    fn count_arg(&self, x: &AbiSimpleEnum) -> u32 {
        match x {
            AbiSimpleEnum::Variant1 => {0}
            AbiSimpleEnum::Variant2(c, _, _, _) => {*c}
            AbiSimpleEnum::Variant3(_, _) => {1}
        }
    }
    fn count_arg_owned(&self, x: AbiSimpleEnum) -> u32 {
        match x {
            AbiSimpleEnum::Variant1 => {0}
            AbiSimpleEnum::Variant2(c, _, _, _) => {c}
            AbiSimpleEnum::Variant3(_, _) => {1}
        }
    }

    fn closure_arg(&self, x: &dyn Fn(&AbiSimpleEnum) -> AbiSimpleEnum) -> AbiSimpleEnum {
        x(&AbiSimpleEnum::Variant1)
    }
}

#[test]
fn check_various_vec_layouts() {
    use savefile::calculate_vec_memory_layout;
    println!("{:?}", calculate_vec_memory_layout::<String>());
    println!("{:?}", calculate_vec_memory_layout::<u8>());
    println!("{:?}", calculate_vec_memory_layout::<u32>());
    println!("{:?}", calculate_vec_memory_layout::<Vec<u8>>());
    println!("{:?}", calculate_vec_memory_layout::<&'static str>());
}

#[test]
fn test_simple_enum_owned() {
    let boxed: Box<dyn SimpleInterfaceWithEnums> = Box::new(Implementation{});
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn SimpleInterfaceWithEnums as AbiExportable>::ABI_ENTRY, boxed).unwrap() };
    assert_eq!(
        conn.count_arg_owned(AbiSimpleEnum::Variant2(42,"hej".into(),"då".into(),"osv".into())),
        42);
}

#[test]
fn test_simple_enum_ref() {
    let boxed: Box<dyn SimpleInterfaceWithEnums> = Box::new(Implementation{});
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn SimpleInterfaceWithEnums as AbiExportable>::ABI_ENTRY, boxed).unwrap() };


    assert_eq!(
        conn.count_arg(&AbiSimpleEnum::Variant2(42,"hej".into(),"då".into(),"osv".into())),
        42);
    let zero : Vec<()> = vec![];
    println!("Mem: {:?}, zero ptr: {:?}", std::mem::size_of::<Vec<()>>(), zero.as_ptr());
    assert_eq!(
        conn.count_arg(&AbiSimpleEnum::Variant3(vec![1,2,3],vec![])),
        1);
}

#[test]
fn test_closure_arg() {
    let boxed: Box<dyn SimpleInterfaceWithEnums> = Box::new(Implementation{});
    let conn = unsafe { AbiConnection::from_boxed_trait(<dyn SimpleInterfaceWithEnums as AbiExportable>::ABI_ENTRY, boxed).unwrap() };

    conn.closure_arg(&|x:&AbiSimpleEnum|x.clone());
}