use savefile::prelude::*;
use savefile::{Introspector, IntrospectorNavCommand, IntrospectedElementKey, IntrospectionError};
use parking_lot::{RwLock, Mutex};


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

#[derive(Savefile)]
pub struct StructWithName {
    #[savefile_introspect_key]
    name: String,
    value: String
}

#[derive(Savefile)]
pub enum EnumWithName {
    Variant1(
        #[savefile_introspect_key]
        String
    ),
    Variant2{#[savefile_introspect_key] name:String, value: String},
    Variant3
}

#[test]
pub fn test_simple_enum_with_key() {
    let var1 = EnumWithName::Variant1("Hejsan".into());
    let var2 = EnumWithName::Variant2 { name:"James".into(), value: "IV".into() };
    let var3 = EnumWithName::Variant3;
    assert_eq!(var1.introspect_len(), 1);
    assert_eq!(var2.introspect_len(), 2);
    assert_eq!(var3.introspect_len(), 0);
    assert_eq!(var1.introspect_value(), "Hejsan");
    assert_eq!(var2.introspect_value(), "James");
    assert_eq!(var3.introspect_value(), "EnumWithName::Variant3");

}

    #[test]
pub fn test_simple_with_key() {
    let val1 = StructWithName {
        name: "Apple".into(),
        value: "Orange".into()
    };
    assert_eq!(val1.introspect_len(), 2);
    assert_eq!(val1.introspect_value(), "Apple");
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
pub fn do_test_rwlock() {
    let test = RwLock::new(SimpleStruct {
        item1: 342
    });

    let _x = (&test).introspect_value();

    assert_eq!(test.introspect_len(), 1);
    assert_eq!(test.introspect_child(0).unwrap().key(), "0");
    assert_eq!(test.introspect_child(0).unwrap().val().introspect_value(), "SimpleStruct");

    let temp = test.introspect_child(0).unwrap();
    let temp2 = temp.val().introspect_child(0).unwrap();
    assert_eq!(temp2.key(),"item1");
    let subchild = temp2.val();
    assert_eq!(subchild.introspect_len(), 0);
    assert_eq!(subchild.introspect_value(), "342");

}

#[test]
pub fn do_test_mutex() {
    let test = Mutex::new(SimpleStruct {
        item1: 343
    });

    let _x = (&test).introspect_value();

    assert_eq!(test.introspect_len(), 1);
    assert_eq!(test.introspect_child(0).unwrap().key(), "0");
    assert_eq!(test.introspect_child(0).unwrap().val().introspect_value(), "SimpleStruct");

    let temp = test.introspect_child(0).unwrap();
    let temp2 = temp.val().introspect_child(0).unwrap();
    assert_eq!(temp2.key(),"item1");
    let subchild = temp2.val();
    assert_eq!(subchild.introspect_len(), 0);
    assert_eq!(subchild.introspect_value(), "343");

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
pub fn test_introspect_no_children() {
    let mut base_introspector = Introspector::new();
    assert_eq!(base_introspector.do_introspect(&0u32, IntrospectorNavCommand::SelectNth{select_depth:0,select_index:0}
    ).unwrap_err(),IntrospectionError::NoChildren);
}
#[test]
pub fn test_introspector_simpler_case0() {
    let comp = ComplexStruct {
        simple1: SimpleStruct {
            item1: 37
        },
        simple2: SimpleStruct {
            item1: 38
        },
        an_int: 4
    };

    let mut base_introspector = Introspector::new();

    let result = base_introspector.do_introspect(&comp, IntrospectorNavCommand::SelectNth{select_depth:0,select_index:0}).unwrap();
    assert_eq!(result.frames.len(),2);
    let result = base_introspector.do_introspect(&comp, IntrospectorNavCommand::SelectNth{select_depth:1,select_index:0}).expect("Leafs should also be selectable");

    assert_eq!(result.frames.len(),2);
    assert_eq!(result.frames[1].keyvals[0].selected,true);


}
#[test]
pub fn test_introspector_simpler_case1() {
    let comp = ComplexStruct {
        simple1: SimpleStruct {
            item1: 37
        },
        simple2: SimpleStruct {
            item1: 38
        },
        an_int: 4
    };

    let mut base_introspector = Introspector::new();

    let result = base_introspector.do_introspect(&comp,
                                                   IntrospectorNavCommand::ExpandElement(
                                                       IntrospectedElementKey {
                                                           key_disambiguator: 0,
                                                           key: "simple2".to_string(),
                                                           depth: 0,
                                                       }
                                                   )).unwrap();

    assert_eq!(result.frames.len(),2);

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
        base_introspector.do_introspect(&comp, IntrospectorNavCommand::Nothing).unwrap();


        assert_eq!(base_introspector.do_introspect(&comp, IntrospectorNavCommand::Up).unwrap_err(),IntrospectionError::AlreadyAtTop);
        assert_eq!(base_introspector.do_introspect(&comp, IntrospectorNavCommand::SelectNth{select_depth:0, select_index:3}).unwrap_err(),IntrospectionError::IndexOutOfRange);
        assert_eq!(base_introspector.do_introspect(&comp, IntrospectorNavCommand::ExpandElement(
            IntrospectedElementKey{
                key: "simple1".into(),
                key_disambiguator: 0,
                depth: 1
            }
        )).unwrap_err(),IntrospectionError::BadDepth);
        assert_eq!(base_introspector.do_introspect(&comp, IntrospectorNavCommand::ExpandElement(
            IntrospectedElementKey{
                key: "simple3".into(),
                key_disambiguator: 0,
                depth: 0
            }
        )).unwrap_err(),IntrospectionError::UnknownKey);

    }
    println!("Base introspector : {:?}",base_introspector);

    let result = base_introspector.do_introspect(&comp, IntrospectorNavCommand::SelectNth{select_depth:0,select_index:0}).unwrap();
    println!("Result: {:?}",result);
    assert_eq!(result.frames.len(),2);
    assert_eq!(result.total_len(), 4);
    assert_eq!(result.total_index(0).unwrap().key.key, "simple1");
    assert_eq!(result.total_index(0).unwrap().value, "SimpleStruct");
    assert_eq!(result.total_index(1).unwrap().key.key, "item1");
    assert_eq!(result.total_index(1).unwrap().value, "37");
    assert_eq!(result.total_index(2).unwrap().key.key, "simple2");
    assert_eq!(result.total_index(2).unwrap().value, "SimpleStruct");
    assert_eq!(result.total_index(3).unwrap().key.key, "an_int");
    assert_eq!(result.total_index(3).unwrap().value, "4");
    assert_eq!(result.total_index(4),None);

    {
        let mut introspector = base_introspector.clone();
        let result = introspector.do_introspect(&comp, IntrospectorNavCommand::Nothing).unwrap();
        assert_eq!(result.frames.len(),2);

        println!("Result debug: {:?}", result);
        let disp = format!("{}",result);

        println!("Distp: {:?}",disp);
        assert!(disp.contains ("*simple1 = SimpleStruct"));
        assert!(disp.contains (">simple2 = SimpleStruct"));
        assert!(disp.contains ("   item1 = 37"));
        assert!(!disp.contains("   item1 = 38")); //Not expanded
        assert!(disp.contains (" an_int = 4"));

        let result3 = introspector.do_introspect(&comp,IntrospectorNavCommand::Up).unwrap();
        assert_eq!(result3.frames.len(),1);
    }

    {
        let mut introspector = base_introspector.clone();
        let result = introspector.do_introspect(&comp, IntrospectorNavCommand::ExpandElement(
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


    let _ = introspector.do_introspect(&sample, IntrospectorNavCommand::SelectNth{select_depth:0, select_index:0}).unwrap();
    let result = introspector.do_introspect(&sample, IntrospectorNavCommand::SelectNth{select_depth:0, select_index:0}).unwrap();
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
