#![no_main]
use libfuzzer_sys::fuzz_target;

extern crate savefile;
#[macro_use]
extern crate savefile_derive;
use savefile::prelude::*;

#[derive(Savefile)]
enum SomeEnum {
    Variant1(usize),
    Variant2{s:isize},
    Variant3
}

#[derive(Savefile)]
struct Simple2(usize,u8);
#[derive(Savefile)]
struct MyUnit();
#[derive(Savefile)]
struct MySimple {
    integer: i8,
    theenum : Option<SomeEnum>,
    strings: Vec<String>,
    simple2: Simple2,
    myunit: MyUnit,
}

fuzz_target!(|data: &[u8]| {
    let mut data = data;
    let _ = load_noschema::<MySimple>(&mut data,0);

});
