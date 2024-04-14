use savefile::prelude::*;
use savefile::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Debug;
use std::io::{BufWriter, Cursor, Write};

pub fn assert_roundtrip<E: Serialize + Deserialize + Debug + PartialEq>(sample: E) {
    assert_roundtrip_version(sample, 0)
}
pub fn assert_roundtrip_version<E: Serialize + Deserialize + Debug + PartialEq>(sample: E, version: u32) {
    let mut f = Cursor::new(Vec::new());
    {
        let mut bufw = BufWriter::new(&mut f);
        {
            Serializer::save(&mut bufw, version, &sample, false).unwrap();
        }
        bufw.flush().unwrap();
    }
    f.set_position(0);
    {
        let roundtrip_result = Deserializer::load::<E>(&mut f, version).unwrap();
        assert_eq!(sample, roundtrip_result);
    }

    let f_internal_size = f.get_ref().len();
    assert_eq!(f.position() as usize, f_internal_size);
}

#[derive(Debug, PartialEq, Savefile)]
enum EnumVer1 {
    Variant1,
    Variant2,
} /*
  #[derive(Savefile,PartialEq,Eq,Debug)]
  struct SimpleStruct {
      x: u32
  }

  #[test]
  fn it_works() {
      assert_roundtrip("Test-string".to_string());
      assert_roundtrip(42i32);
      assert_roundtrip(SimpleStruct{x:42});
  }
  */
