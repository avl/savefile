use savefile_abi::AbiConnection;
use savefile_abi::AbiExportable;
use std::collections::HashMap;

#[savefile_abi_exportable(version = 0)]
pub trait SimpleInterface {
    fn do_call(&self, x: u32) -> u32;
}
#[savefile_abi_exportable(version = 0)]
pub trait AdvancedTestInterface {
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String>;
    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String>;

    fn return_trait_object(&self) -> Box<dyn SimpleInterface>;
    fn test_slices(&mut self, slice: &[u32]) -> u32;

    fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32>;
    fn return_boxed_closure2(&self) -> Box<dyn Fn()>;
    fn many_callbacks(&mut self, x: &mut dyn FnMut(&dyn Fn(&dyn Fn() -> u32) -> u32) -> u32) -> u32;
}
struct SimpleImpl;

impl Drop for SimpleImpl {
    fn drop(&mut self) {
        println!("Dropping impl")
    }
}
impl SimpleInterface for SimpleImpl {
    fn do_call(&self, x: u32) -> u32 {
        println!("do_call running");
        x
    }
}
struct AdvancedTestInterfaceImpl {}

impl AdvancedTestInterface for AdvancedTestInterfaceImpl {
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String> {
        x
    }

    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String> {
        x.clone()
    }

    fn return_trait_object(&self) -> Box<dyn SimpleInterface> {
        Box::new(SimpleImpl)
    }

    fn return_boxed_closure(&self) -> Box<dyn Fn() -> u32> {
        Box::new(|| 42)
    }
    fn return_boxed_closure2(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    fn test_slices(&mut self, slice: &[u32]) -> u32 {
        slice.iter().copied().sum()
    }

    fn many_callbacks(&mut self, x: &mut dyn FnMut(&dyn Fn(&dyn Fn() -> u32) -> u32) -> u32) -> u32 {
        x(&|y| y())
    }
}

#[test]
fn abi_test_slice() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    assert!(conn.get_arg_passable_by_ref("test_slices", 0));
    assert_eq!(conn.test_slices(&[1, 2, 3, 4]), 10);
}

#[test]
fn test_trait_object_in_return_position() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let ret = conn.return_trait_object();
    assert_eq!(ret.do_call(42), 42);
    assert_eq!(ret.do_call(42), 42);
}
#[test]
fn test_return_boxed_closure() {
    let closure;
    let closure2;
    {
        let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
        let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

        closure = conn.return_boxed_closure();
        closure2 = conn.return_boxed_closure2();
        assert_eq!(closure(), 42);
    }
    assert_eq!(closure(), 42);
    closure2();
}

#[test]
fn test_call_many_callbacks() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let mut conn = AbiConnection::from_boxed_trait(boxed).unwrap();
    assert_eq!(
        conn.many_callbacks(&mut |x| {
            x(&|| {
                println!("In the inner sanctum!");
                42
            })
        }),
        42
    );
}
#[test]
fn test_advanced_abi2() {
    let boxed: Box<dyn AdvancedTestInterface> = Box::new(AdvancedTestInterfaceImpl {});
    let conn = AbiConnection::from_boxed_trait(boxed).unwrap();

    let mut mymap = HashMap::new();
    mymap.insert("mascot".to_string(), "ferris".to_string());
    mymap.insert("concurrency".to_string(), "fearless".to_string());
    let mymap = conn.roundtrip_hashmap(mymap);

    let mymap2: HashMap<String, String> = conn.clone_hashmap(&mymap);

    assert!(mymap2.contains_key("mascot"));
    assert_eq!(mymap2["mascot"], "ferris");
}
