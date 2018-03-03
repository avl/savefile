use ::savefile::prelude::*;



#[derive(ReprC, Clone, Copy, Debug, PartialEq, Savefile)]
struct Inner {
	misaligner : u8, 
	x: u32
}



#[test]
#[should_panic] //Inner struct is not packed (same in memory as on disk)
#[cfg(debug_assertions)] //This test only works in debug builds
fn test_not_raw_memcpy1() {
    use std::io::Cursor;
	let sample  = vec![	
        Inner { misaligner:0, x: 32}
	];

    let mut f = Cursor::new(Vec::new());
    {
        Serializer::save_noschema(&mut f, 0, &sample).unwrap(); //Should panic here, Inner contains padding.
    }
}