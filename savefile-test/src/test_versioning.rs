use ::savefile::prelude::*;

#[derive(Debug, PartialEq, Savefile)]
struct Version1 {
	a: String,
	b: Vec<String>,
	c: usize
}
#[derive(Debug, PartialEq, Savefile)]
struct Version2 {
	a: String,		
    #[versions = "0..0"]
	b: Removed<Vec<String>>,
    #[default_val = "123"]
    #[versions = "1.."]
	newb: u32,
	c: usize
}

#[derive(Debug, PartialEq, Savefile)]
struct Version3 {
	a: String,		
    #[versions = "0..0"]
	b: Removed<Vec<String>>,
    #[versions = "1..1"]
	newb: u32,
	c: usize,
    #[versions = "2.."]
	d: usize
}



#[test]
fn simple_vertest1() {
    use ::assert_roundtrip_to_new_version;
	let ver2:Version2 = assert_roundtrip_to_new_version(
		Version1 {
			a: "Hello".to_string(),
			b: vec!["a".to_string(),"b".to_string()],
			c: 412
		},
		0,
		Version2 {
			a: "Hello".to_string(),
			b: Removed::new(),
			newb: 123,
			c: 412
		},
		1
		);

	assert_roundtrip_to_new_version(
		ver2,
		1,
		Version3 {
			a: "Hello".to_string(),
			b: Removed::new(),
			newb: 123,
			c: 412,
			d: 0
		},
		2
		);

}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVer1 {
    Variant1,
    Variant2,
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVer2 {
    Variant1,
    Variant2,
    #[versions = "1.."]
    Variant3,
}

#[test]
fn test_versioning_of_enums() {
    use ::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        EnumVer1::Variant1,
        0,
        EnumVer2::Variant1,
        1
        );
    assert_roundtrip_to_new_version(
        EnumVer1::Variant2,
        0,
        EnumVer2::Variant2,
        1
        );

}

#[derive(Debug, PartialEq, Savefile )]
enum EnumVerA1 {
    Variant1,
    Variant2{x:u32,y:u32},
}

#[derive(Debug, PartialEq, Savefile )]
enum EnumVerA2 {
    Variant1,
    Variant2 {
    	x:u32,
    	#[versions = "0..0"]    	
    	y:Removed<u32>
    },
}

#[test]
fn test_versioning_of_enums2() {
    use ::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        EnumVerA1::Variant2{x:32,y:33},
        0,
        EnumVerA2::Variant2{x:32,y:Removed::new()},
        1
        );

}


#[derive(Debug, PartialEq, Savefile)]
enum EnumVerB1 {
    Variant1,
    Variant2(u32,u32),
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVerB2 {
    Variant1,
    Variant2(
    	u32,
    	#[versions = "0..0"]    	
    	Removed<u32>
    ),
}

#[test]
fn test_versioning_of_enums3() {
    use ::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        EnumVerB1::Variant2(32,33),
        0,
        EnumVerB2::Variant2(32,Removed::new()),
        1
        );

}


#[derive(Debug, PartialEq, Savefile)]
struct SubSubData1 {
	x:u32
}
#[derive(Debug, PartialEq, Savefile)]
struct SubData1 {
	some_sub : SubSubData1,
}
#[derive(Debug, PartialEq, Savefile)]
struct ComplexData1 {
	some_field : SubData1,
}

#[derive(Debug, PartialEq, Savefile)]
struct SubSubData2 {
	x:u32
}
#[derive(Debug, PartialEq, Savefile)]
struct SubData2 {
	some_sub : SubSubData2,	
}
#[derive(Debug, PartialEq, Savefile)]
struct ComplexData2 {
	some_field : SubData2,
}

#[test]
fn test_versioning_of_enums4() {
    use ::assert_roundtrip_to_new_version;
    assert_roundtrip_to_new_version(
        ComplexData1{some_field:SubData1{some_sub: SubSubData1{x:43}}},
        0,
        ComplexData2{some_field:SubData2{some_sub: SubSubData2{x:43}}},
        1
        );

}

#[derive(Debug, PartialEq, Savefile)]
enum DefTraitEnum {
	VariantA,
	VariantB,
	VariantC,
}

impl Default for DefTraitEnum {
	fn default() -> DefTraitEnum {
		DefTraitEnum::VariantA
	}
}

#[derive(Debug, PartialEq, Savefile)]
struct DefTraitTest {
    #[versions = "0..0"]    		
    removed_enum:DefTraitEnum
}

#[test]
fn test_default_trait1() {
	use ::assert_roundtrip_version;
	assert_roundtrip_version::<DefTraitTest>(
		DefTraitTest {
			removed_enum : DefTraitEnum::VariantA
		},1);
}
