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

#[cfg(feature = "nightly")]
//The whole system to use a faster serialization/deserialization for Vec<T> where T:ReprC only works on nightly (since it depends on specialisation)
#[test]
#[cfg(debug_assertions)] //This test only works in debug builds
fn test_misaligned1() {
    assert_eq!(unsafe { Inner::repr_c_optimization_safe(0).is_yes() }, false);
    assert_eq!(unsafe { CorrectlyAligned::repr_c_optimization_safe(0).is_yes() }, true);
}

#[derive(Clone, Copy, Debug, PartialEq, Savefile)]
//#[savefile_unsafe_and_fast] This now causes compilation failure, so test doesn't really work
struct Inner2 {
    x: u32,
    misaligner: u8,
}

#[cfg(feature = "nightly")]
//The whole system to use a faster serialization/deserialization for Vec<T> where T:ReprC only works on nightly (since it depends on specialisation)
#[test]
#[cfg(debug_assertions)] //This test only works in debug builds
fn test_misaligned2() {
    assert_eq!(unsafe { Inner2::repr_c_optimization_safe(0).is_yes() }, false);
    assert_eq!(unsafe { CorrectlyAligned::repr_c_optimization_safe(0).is_yes() }, true);
}
