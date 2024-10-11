use arrayvec::ArrayVec;
use crate::assert_roundtrip;

#[test]
pub fn test_arrayvec0() {
    let a = ArrayVec::<(), 1>::new();
    assert_roundtrip(a);
}

#[test]
pub fn test_arrayvec1() {
    let mut a = ArrayVec::<i32, 2>::new();
    a.push(43i32);
    assert_roundtrip(a);
}
#[test]
pub fn test_arrayvec2() {
    let mut a = ArrayVec::<i32, 128>::new();
    for _ in 0..100 {
        a.push(43i32);
    }
    assert_roundtrip(a);
}
#[test]
pub fn test_arrayvec3() {
    let mut a = ArrayVec::<_, 128>::new();
    for _ in 0..64 {
        a.push("Hello guys".to_string());
        a.push("Hello again".to_string());
    }
    assert_roundtrip(a);
}
#[test]
pub fn test_arrayvec4() {
    let a = ArrayVec::<String, 128>::new();
    assert_roundtrip(a);
}
