use std::borrow::Cow;
use std::io::Cursor;
use savefile::{load, save, Packed, Serialize};


#[derive(Savefile, PartialEq, Eq, Clone)]
struct MaybeSerializable<T> {
    field: Vec<T>
}

impl<T:Serialize+Packed> MaybeSerializable<T> {
    fn save(&self, buf: &mut Vec<u8>) {
        save(buf, 0, self).unwrap();
    }
}

#[test]
fn test_serialize_maybe_serializable() {
    let mut temp = Vec::new();
    {
        let temp_val = 42u32;
        let example = MaybeSerializable {
            field: vec![Cow::Borrowed(&temp_val)]
        };

        example.save(&mut temp)
    }

    let roundtripped: MaybeSerializable<u32> = load(&mut Cursor::new(temp), 0).unwrap();

    assert_eq!(roundtripped.field, vec![42]);
}

#[test]
fn test_serialize_non_static() {
    let mut temp = Vec::new();
    {
        let x = 42u32;
        let non_static = Cow::Borrowed(&x);
        save(&mut temp, 0, &non_static).unwrap();
    }

    let roundtripped: Cow<'static, u32> = load(&mut Cursor::new(temp), 0).unwrap();

    assert_eq!(*roundtripped, 42u32);
}
#[test]
fn test_serialize_non_static2() {
    let mut temp = Vec::new();
    {
        let x = 42u32;
        let non_static = Cow::Borrowed(&x);
        save(&mut temp, 0, &non_static).unwrap();
    }

    let roundtripped: Cow<'static, u32> = load(&mut Cursor::new(temp), 0).unwrap();

    assert_eq!(*roundtripped, 42u32);
}

fn test_serialize_non_static_with_lifetime<'a>(x: &'a u32) {
    let mut temp = Vec::new();
    {
        let non_static = Cow::<'a, u32>::Borrowed(x);
        save(&mut temp, 0, &non_static).unwrap();
    }

    let roundtripped: Cow<'a, u32> = load(&mut Cursor::new(temp), 0).unwrap();

    assert_eq!(*roundtripped, 43u32);
}


#[test]
fn test_with_specific_lifetime() {
    let x = 43u32;
    test_serialize_non_static_with_lifetime(&x);
}

