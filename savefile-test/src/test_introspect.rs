use savefile::prelude::*;
use savefile::{Introspector, IntrospectorNavCommand, IntrospectedElementKey, IntrospectionError};


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
    simple1: SimpleStruct,
    simple2: SimpleStruct,
    an_int: u32
}



#[test]
pub fn test_simple_enum() {
    let val1 = SimpleEnum::VariantA(11,12,13);
    assert_eq!(val1.introspect_len(), 3);
    assert_eq!(val1.introspect_child(0).unwrap().key(), "0");
    assert_eq!(val1.introspect_child(0).unwrap().val().introspect_value(), "11");
    assert_eq!(val1.introspect_child(1).unwrap().key(), "1");
    assert_eq!(val1.introspect_child(1).unwrap().val().introspect_value(), "12");
    assert_eq!(val1.introspect_child(2).unwrap().key(), "2");
    assert_eq!(val1.introspect_child(2).unwrap().val().introspect_value(), "13");

    let val2 = SimpleEnum::VariantB{x:74,y:32};
    assert_eq!(val2.introspect_len(), 2);
    assert_eq!(val2.introspect_child(0).unwrap().key(), "x");
    assert_eq!(val2.introspect_child(0).unwrap().val().introspect_value(), "74");
    assert_eq!(val2.introspect_child(1).unwrap().key(), "y");
    assert_eq!(val2.introspect_child(1).unwrap().val().introspect_value(), "32");

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
    assert_eq!(x, "SimpleStruct");

    assert_eq!(test.introspect_len(), 1);
    assert_eq!(test.introspect_child(0).unwrap().key(), "item1");
    assert_eq!(test.introspect_child(0).unwrap().val().introspect_value(), "342");

}


#[test]
pub fn func_to_do_stuff() {

    let os = OtherStruct(43,32);

    assert_eq!(os.introspect_len(), 2);
    assert_eq!(os.introspect_child(0).unwrap().key(), "0");
    assert_eq!(os.introspect_child(0).unwrap().val().introspect_value(), "43");
    assert_eq!(os.introspect_child(1).unwrap().key(), "1");
    assert_eq!(os.introspect_child(1).unwrap().val().introspect_value(), "32");
}


#[test]
pub fn test_introspector_simple_case() {
    let comp = ComplexStruct {
        simple1: SimpleStruct {
            item1 : 37
        },
        simple2: SimpleStruct {
            item1 : 38
        },
        an_int: 4
    };

    let mut base_introspector = Introspector::new();

    {
        base_introspector.impl_get_frames(&comp, IntrospectorNavCommand::Nothing).unwrap();


        assert_eq!(base_introspector.impl_get_frames(&comp, IntrospectorNavCommand::Up).unwrap_err(),IntrospectionError::AlreadyAtTop);
        assert_eq!(base_introspector.impl_get_frames(&comp, IntrospectorNavCommand::SelectNth(3)).unwrap_err(),IntrospectionError::IndexOutOfRange);
        assert_eq!(base_introspector.impl_get_frames(&comp, IntrospectorNavCommand::ExpandElement(
            IntrospectedElementKey{
                key: "simple1".into(),
                key_disambiguator: 0,
                depth: 1
            }
        )).unwrap_err(),IntrospectionError::BadDepth);
        assert_eq!(base_introspector.impl_get_frames(&comp, IntrospectorNavCommand::ExpandElement(
            IntrospectedElementKey{
                key: "simple3".into(),
                key_disambiguator: 0,
                depth: 0
            }
        )).unwrap_err(),IntrospectionError::UnknownKey);

        assert_eq!(base_introspector.impl_get_frames(&0u32, IntrospectorNavCommand::SelectNth(
            0
        )).unwrap_err(),IntrospectionError::NoChildren);
    }

    let result = base_introspector.impl_get_frames(&comp, IntrospectorNavCommand::SelectNth(0)).unwrap();
    assert_eq!(result.frames.len(),2);



    {
        let mut introspector = base_introspector.clone();
        let result = introspector.impl_get_frames(&comp, IntrospectorNavCommand::Nothing).unwrap();
        assert_eq!(result.frames.len(),2);

        let disp = format!("{}",result);

        assert!(disp.contains ("*simple1 = SimpleStruct"));
        assert!(disp.contains (">simple2 = SimpleStruct"));
        assert!(disp.contains ("   item1 = 37"));
        assert!(!disp.contains("   item1 = 38")); //Not expanded
        assert!(disp.contains (" an_int = 4"));

        let result3 = introspector.impl_get_frames(&comp,IntrospectorNavCommand::Up).unwrap();
        assert_eq!(result3.frames.len(),1);
    }

    {
        let mut introspector = base_introspector.clone();
        let result = introspector.impl_get_frames(&comp, IntrospectorNavCommand::ExpandElement(
            IntrospectedElementKey{
                depth: 0,
                key: "simple2".to_string(),
                .. Default::default()
            })).unwrap();
        let disp = format!("{}",result);
        assert_eq!(result.frames.len(),2);
        assert!(disp.contains(">simple1 = SimpleStruct"));
        assert!(disp.contains("*simple2 = SimpleStruct"));
        assert!(!disp.contains("   item1 = 37")); //Now expanded
        assert!(disp.contains("   item1 = 38")); //Now expanded

    }
}

#[derive(Savefile)]
struct NestableStruct {
    a: Option<Box<NestableStruct>>,
    b: Option<Box<NestableStruct>>,
    c: u32,
}

#[test]
pub fn test_introspector_deeply_nested_case() {
    let sample = NestableStruct {
        a: Some(Box::new(NestableStruct {
            a: Some(Box::new(NestableStruct {
                a:None,b:None,c:1
            })),
            b: Some(Box::new(NestableStruct {
                a:None,b:None,c:2
            })),
            c:3
        })),
        b: Some(Box::new(NestableStruct {
            a: Some(Box::new(NestableStruct {
                a:None,b:None,c:4
            })),
            b: Some(Box::new(NestableStruct {
                a:None,b:None,c:5
            })),
            c:6
        })),
        c:7
    };

    let mut introspector = Introspector::new();


    let _ = introspector.impl_get_frames(&sample, IntrospectorNavCommand::SelectNth(0)).unwrap();
    let result = introspector.impl_get_frames(&sample, IntrospectorNavCommand::SelectNth(0)).unwrap();
    assert_eq!(result.frames[0].keyvals[0].key.key,"a");
    assert_eq!(result.frames[0].keyvals[1].key.key,"b");
    assert_eq!(result.frames[0].keyvals[2].key.key,"c");
    assert_eq!(result.frames[0].keyvals[2].value,"7");

    let disp = format!("{}",result);
    assert_eq!(disp,"Introspectionresult:
*a = Some(NestableStruct)
  *a = Some(NestableStruct)
     a = None
     b = None
     c = 1
  >b = Some(NestableStruct)
   c = 3
>b = Some(NestableStruct)
 c = 7
");

}