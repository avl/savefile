
use ::savefile::prelude::*;

#[derive(ReprC, Clone, Copy, Debug, PartialEq, Savefile)]
struct Inner {
	x: u32
}

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
struct Nested {
    misaligner : u8, 
	inner : Inner
}


#[derive(ReprC, Clone, Copy, Debug, PartialEq, Savefile)]
#[repr(u8)]
pub enum TestReprEnum {
    A,
    B,
    C,
}

#[test]
fn test_not_raw_memcpy2() {
    use std::io::Cursor;
	let sample  = vec![	
		Nested { misaligner:0, inner: Inner { x: 32}}
	];

    let mut f = Cursor::new(Vec::new());
    {
        Serializer::save_noschema(&mut f, 0, &sample).unwrap();
    }

    let f_internal_size = f.get_ref().len();

    let vec_overhead=8;
    let version=4;
    let misaligner=1;
    let inner=4;
    assert_eq!(f_internal_size, version + vec_overhead + misaligner + inner ); //3 bytes padding also because of ReprC-optimization
}


#[derive(Savefile,Clone,Copy,ReprC)]
struct MyUnitStruct {

}

#[derive(Savefile,Clone,Copy,ReprC)]
struct UnnamedFieldsStruct(usize);


#[test]
fn test_various_types_for_reprc() {

    assert_eq!(<() as ReprC>::repr_c_optimization_safe(0), true);
    assert_eq!(<u8>::repr_c_optimization_safe(0),true);

    assert_eq!(<MyUnitStruct>::repr_c_optimization_safe(0), true);
    assert_eq!(UnnamedFieldsStruct::repr_c_optimization_safe(0), true);


}