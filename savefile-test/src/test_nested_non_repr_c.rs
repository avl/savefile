#![cfg(test)]
use roundtrip;
use savefile::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
#[savefile_unsafe_and_fast]
struct CorrectlyAligned {
    //Used as a known-good to compare to
    x: u32,
    y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
//#[savefile_unsafe_and_fast] This now causes compilation failure, so test doesn't really work
struct Inner {
    misaligner: u8,
    x: u32,
}

#[test]
#[cfg(debug_assertions)] //This test only works in debug builds
fn test_misaligned1() {
    assert_eq!(unsafe { Inner::repr_c_optimization_safe(0).is_yes() }, false);
    assert_eq!(unsafe { CorrectlyAligned::repr_c_optimization_safe(0).is_yes() }, true);
}

#[test]
fn roundtrip_correctly_aligned() {
    roundtrip(CorrectlyAligned{
        x: 1, y: 2
    });
    roundtrip(Inner{
        misaligner: 43,
        x: 42
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
//#[savefile_unsafe_and_fast] This now causes compilation failure, so test doesn't really work
struct Inner2 {
    x: u32,
    misaligner: u8,
}

#[test]
#[cfg(debug_assertions)] //This test only works in debug builds
fn test_misaligned2() {
    assert_eq!(unsafe { Inner2::repr_c_optimization_safe(0).is_yes() }, false);
    assert_eq!(unsafe { CorrectlyAligned::repr_c_optimization_safe(0).is_yes() }, true);
}

#[test]
fn test_roundtrip_inner2() {
    roundtrip(Inner2 {
        x: 47,
        misaligner: 48
    });
}
