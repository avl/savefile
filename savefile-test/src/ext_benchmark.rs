use rand::Rng;
use std::hint::black_box;
#[cfg(feature = "nightly")]
use test::Bencher;
use savefile::TIGHT;

mod savefile_test_bad_schema {
    use savefile::prelude::*;

    #[derive(Savefile, PartialEq, Debug)]
    struct Original {
        some_number: usize,
        a_few_strings: Vec<String>,
    }

    #[derive(Savefile, PartialEq, Debug)]
    struct NewVersion {
        a_few_strings: Vec<String>,
        some_number: usize,
    }

    #[test]
    #[should_panic(
        expected = "called `Result::unwrap()` on an `Err` value: IncompatibleSchema { message: \"Saved schema differs from in-memory schema for version 0. Error: At location [./Original/some_number]: In memory schema: vector, file schema: primitive\" }"
    )]
    fn test_schema_mismatch_savefile() {
        let original = Original {
            some_number: 0,
            a_few_strings: vec!["hello".to_string()],
        };

        let encoded: Vec<u8> = save_to_mem(0, &original).unwrap();
        let decoded: NewVersion = load_from_mem(&encoded[..], 0).unwrap();
        println!("Savefile decoded: {:?}", decoded);
    }
}

mod bincode_test_bad_schema {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Original {
        some_number: usize,
        a_few_strings: Vec<String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct NewVersion {
        a_few_strings: Vec<String>,
        some_number: usize,
    }

    #[test]
    fn test_schema_mismatch_bincode() {
        let original = Original {
            some_number: 0,
            a_few_strings: vec!["hello".to_string()],
        };

        let encoded: Vec<u8> = bincode::serialize(&original).unwrap();
        let decoded: NewVersion = bincode::deserialize(&encoded[..]).unwrap();
        println!("Bincode decoded: {:?}", decoded);
    }
}

#[cfg(feature = "nightly")]
mod bincode_benchmark {
    use serde::{Deserialize, Serialize};
    use test::{black_box, Bencher};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Entity {
        x: f32,
        y: f32,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct World(Vec<Entity>);

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_ext_bincode(b: &mut Bencher) {
        let mut entities = Vec::new();
        for _ in 0..100_000 {
            entities.push(Entity { x: 0.0, y: 4.0 });
            entities.push(Entity { x: 10.0, y: 20.5 });
        }
        let world = World(entities);
        b.iter(move || {
            let encoded: Vec<u8> = bincode::serialize(&world).unwrap();

            // 8 bytes for the length of the vector, 4 bytes per float.
            //assert_eq!(encoded.len(), 8 + 4 * 4);

            let decoded: World = bincode::deserialize(&encoded[..]).unwrap();

            //assert_eq!(world, decoded);
            decoded
        })
    }
}

#[cfg(feature = "nightly")]
mod savefile_benchmark {
    use savefile::prelude::*;
    use test::{black_box, Bencher};

    #[derive(Savefile, PartialEq, Debug, Clone, Copy)]
    #[savefile_require_fast]
    #[repr(C)]
    struct Entity {
        x: f32,
        y: f32,
    }

    #[derive(Savefile, PartialEq, Debug)]
    struct World(Vec<Entity>);

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_ext_savefile_with_reprc(b: &mut Bencher) {
        let mut entities = Vec::new();
        for _ in 0..100_000 {
            entities.push(Entity { x: 0.0, y: 4.0 });
            entities.push(Entity { x: 10.0, y: 20.5 });
        }
        let world = World(entities);

        b.iter(move || {
            let mut encoded = Vec::new();
            savefile::save(&mut encoded, 0, &world).unwrap();

            let mut encoded_slice = &encoded[..];
            let decoded: World = savefile::load::<World>(&mut encoded_slice, 0).unwrap();

            assert!((decoded.0.last().unwrap().x - 10.0).abs() < 1e-9);
            decoded
        })
    }
}

#[cfg(feature = "nightly")]
mod savefile_benchmark_no_reprc {
    use savefile::prelude::*;
    use test::{black_box, Bencher};

    #[derive(Savefile, PartialEq, Debug, Clone, Copy)]
    struct Entity {
        x: f32,
        y: f32,
    }

    #[derive(Savefile, PartialEq, Debug)]
    struct World(Vec<Entity>);

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_ext_savefile_no_reprc(b: &mut Bencher) {
        let mut entities = Vec::new();
        for _ in 0..100_000 {
            entities.push(Entity { x: 0.0, y: 4.0 });
            entities.push(Entity { x: 10.0, y: 20.5 });
        }
        let world = World(entities);

        b.iter(move || {
            let mut encoded = Vec::new();
            savefile::save(&mut encoded, 0, &world).unwrap();

            let mut encoded_slice = &encoded[..];
            let decoded: World = savefile::load::<World>(&mut encoded_slice, 0).unwrap();

            assert!((decoded.0.last().unwrap().x - 10.0).abs() < 1e-9);
            decoded
        })
    }
}

#[derive(Savefile, PartialEq, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
#[derive(Savefile, PartialEq, Default)]
pub struct Triangle {
    pub v0: Vector3,
    pub v1: Vector3,
    pub v2: Vector3,
    pub normal: Vector3,
}

#[derive(Savefile, PartialEq)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
}
#[cfg(test)]
pub fn generate_mesh() -> Mesh {
    let mut mesh = Mesh { triangles: vec![] };
    const TRIANGLES: usize = 125_000;
    for _ in 0..TRIANGLES {
        mesh.triangles.push(Triangle::default())
    }

    mesh
}
#[cfg(feature = "nightly")]
#[bench]
fn bench_ext_triangle(b: &mut Bencher) {
    let mesh = generate_mesh();
    let mut encoded: Vec<u8> = Vec::new();
    b.iter(move || {
        encoded.clear();
        savefile::save_noschema(black_box(&mut encoded), 0, black_box(&mesh)).unwrap();
    })
}
#[test]
fn test_triangle() {
    use savefile::Packed;
    if !TIGHT {
        assert!(unsafe { Triangle::repr_c_optimization_safe(0).is_yes() });
    }
    let mesh = generate_mesh();

    let mut encoded = Vec::new();
    encoded.clear();
    savefile::save_noschema(black_box(&mut encoded), 0, black_box(&mesh)).unwrap();
}
