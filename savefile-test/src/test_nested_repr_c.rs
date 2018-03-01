use ::savefile::prelude::*;
use std::io::Cursor;


#[derive(WithSchema, ReprC, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Inner {
	misaligner : u8, 
	x: u32
}

#[derive(WithSchema, ReprC, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Nested {
	inner : Inner
}



#[test]
fn test_not_raw_memcpy() {
	let mut sample  = vec![	
		Nested { inner: Inner { misaligner:0, x: 32}}
	];

    let mut f = Cursor::new(Vec::new());
    {
        let mut serializer = Serializer::store_noschema(&mut f, 0, &sample);
    }

    let f_internal_size = f.get_ref().len();
    assert_eq!(f_internal_size, 8 + 4 + 4 + 1 + 3 ); //3 bytes padding also because of ReprC-optimization
}