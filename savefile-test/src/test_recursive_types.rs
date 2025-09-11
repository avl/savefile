use crate::roundtrip;
use crate::savefile::WithSchema;
use insta::assert_debug_snapshot;
use savefile::WithSchemaContext;

#[derive(Savefile, Debug)]
struct Relay {
    relay: Box<RecursiveType>,
}

#[derive(Savefile, Debug)]
struct RecursiveType {
    left: Option<Box<RecursiveType>>,
    right: Option<Box<RecursiveType>>,
    mid: Vec<Relay>,
}

#[test]
#[cfg(not(miri))]
fn get_recursive_schema() {
    let mut temp = WithSchemaContext::new();
    let schema = RecursiveType::schema(0, &mut temp);
    println!("Schema: {:#?}", schema);
    assert_debug_snapshot!(schema);
}

#[test]
fn roundtrip_recursive_type() {
    let value = RecursiveType {
        left: Some(Box::new(RecursiveType {
            left: None,
            right: None,
            mid: vec![],
        })),
        right: None,
        mid: vec![Relay {
            relay: Box::new(RecursiveType {
                left: None,
                right: None,
                mid: vec![],
            }),
        }],
    };
    roundtrip(value);
}
