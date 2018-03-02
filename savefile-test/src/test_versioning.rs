use ::savefile::prelude::*;

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct Version1 {
	a: String,
	b: Vec<String>,
	c: usize
}
#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct Version2 {
	a: String,		
    #[versions = "0..0"]
	b: Removed<Vec<String>>,
    #[default_val = "123"]
    #[versions = "1.."]
	newb: u32,
	c: usize
}

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
struct Version3 {
	a: String,		
    #[versions = "0..0"]
	b: Removed<Vec<String>>,
    #[versions = "1..1"]
	newb: u32,
	c: usize,
	#[default_val = "37"]
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
			d: 37
		},
		2
		);

}

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
enum EnumVer1 {
    Variant1,
    Variant2,
}

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
enum EnumVerA1 {
    Variant1,
    Variant2{x:u32,y:u32},
}

#[derive(Debug, WithSchema, PartialEq, Serialize, Deserialize)]
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
