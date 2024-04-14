use savefile_abi::AbiConnection;
use savefile_abi::AbiExportable;
use std::collections::HashMap;

#[savefile_abi_exportable(version = 0)]
pub trait AdvancedTestInterface {
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String>;
    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String>;
}

struct AdvancedTestInterfaceImpl {}

impl AdvancedTestInterface for AdvancedTestInterfaceImpl {
    fn roundtrip_hashmap(&self, x: HashMap<String, String>) -> HashMap<String, String> {
        x
    }

    fn clone_hashmap(&self, x: &HashMap<String, String>) -> HashMap<String, String> {
        x.clone()
    }
}

#[test]
fn test_abi_removed_with_custom_default() {
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
