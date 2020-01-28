use savefile::prelude::*;
use savefile::{Introspector, IntrospectorNavCommand};


#[derive(Savefile)]
pub struct OtherStruct(u32,u16);

#[derive(Savefile)]
pub struct SillyStruct{}



#[derive(Savefile)]
pub enum SimpleEnum {
    VariantA(u32,u16,u8),
    VariantB{x:i32,y:i8},
    VariantC
}

#[derive(Savefile)]
pub struct SimpleStruct {
    item1: u32
}

#[derive(Savefile)]
pub struct ComplexStruct {
    simple: SimpleStruct,
    an_int: u32
}



#[test]
pub fn test_simple_enum() {
    let val1 = SimpleEnum::VariantA(11,12,13);
    assert_eq!(val1.introspect_len(), 3);
    assert_eq!(val1.introspect_child(0).unwrap().0, "0");
    assert_eq!(val1.introspect_child(0).unwrap().1.introspect_value(), "11");
    assert_eq!(val1.introspect_child(1).unwrap().0, "1");
    assert_eq!(val1.introspect_child(1).unwrap().1.introspect_value(), "12");
    assert_eq!(val1.introspect_child(2).unwrap().0, "2");
    assert_eq!(val1.introspect_child(2).unwrap().1.introspect_value(), "13");

    let val2 = SimpleEnum::VariantB{x:74,y:32};
    assert_eq!(val2.introspect_len(), 2);
    assert_eq!(val2.introspect_child(0).unwrap().0, "x");
    assert_eq!(val2.introspect_child(0).unwrap().1.introspect_value(), "74");
    assert_eq!(val2.introspect_child(1).unwrap().0, "y");
    assert_eq!(val2.introspect_child(1).unwrap().1.introspect_value(), "32");

    let val3 = SimpleEnum::VariantC;
    assert_eq!(val3.introspect_len(), 0);
    assert_eq!(val3.introspect_value(), "SimpleEnum::VariantC");

}

#[test]
pub fn do_test_silly_struct() {
    let test = SillyStruct{};
    assert_eq!(test.introspect_value(), "SillyStruct");
    assert_eq!(test.introspect_len(), 0);
}

#[test]
pub fn do_test1() {
    let test = SimpleStruct {
        item1: 342
    };

    let x = (&test).introspect_value();
    println!("X: {:?}",x);
    assert_eq!(test.introspect_len(), 1);
    assert_eq!(test.introspect_child(0).unwrap().0, "item1");
    assert_eq!(test.introspect_child(0).unwrap().1.introspect_value(), "342");

}


#[test]
pub fn func_to_do_stuff() {

    let os = OtherStruct(43,32);

    assert_eq!(os.introspect_len(), 2);
    assert_eq!(os.introspect_child(0).unwrap().0, "0");
    assert_eq!(os.introspect_child(0).unwrap().1.introspect_value(), "43");
    assert_eq!(os.introspect_child(1).unwrap().0, "1");
    assert_eq!(os.introspect_child(1).unwrap().1.introspect_value(), "32");
}


#[test]
pub fn test_introspector() {
    let comp = ComplexStruct {
        simple: SimpleStruct {
            item1 : 37
        },
        an_int: 4
    };

    let mut introspector = Introspector::new();

    let result = introspector.impl_get_frames(&comp, IntrospectorNavCommand::SelectNth(0));
    println!("Result1: {:?}",result);
    let result = introspector.impl_get_frames(&comp, IntrospectorNavCommand::Nothing);
    println!("Result2: {:?}",result);
}