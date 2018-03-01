use ::savefile::prelude::*;
use ::assert_roundtrip_to_new_version;
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Version1 {
	a: String,
	b: Vec<String>,
	c: usize
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Version2 {
	a: String,		
    #[versions = "0..0"]
	oldb: Removed<Vec<String>>,
    #[default_val = "123"]
    #[versions = "1.."]
	b: u32,
	c: usize
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Version3 {
	a: String,		
    #[versions = "0..0"]
	oldb: Removed<Vec<String>>,
    #[versions = "1..1"]
	b: u32,
	c: usize,
	#[default_val = "37"]
    #[versions = "2.."]
	d: usize
}



#[test]
fn simple_vertest1() {
	let ver2:Version2 = assert_roundtrip_to_new_version(
		Version1 {
			a: "Hello".to_string(),
			b: vec!["a".to_string(),"b".to_string()],
			c: 412
		},
		0,
		Version2 {
			a: "Hello".to_string(),
			oldb: Removed::new(),
			b: 123,
			c: 412
		},
		1
		);

	assert_roundtrip_to_new_version(
		ver2,
		1,
		Version3 {
			a: "Hello".to_string(),
			oldb: Removed::new(),
			b: 123,
			c: 412,
			d: 37
		},
		2
		);



}
