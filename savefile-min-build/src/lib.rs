#![allow(soft_unstable)]
#![feature(test)]
extern crate alkahest;

extern crate test;

use std::hint::black_box;
use test::Bencher;


use savefile_derive::Savefile;
use savefile::Packed;

#[derive(Clone, Copy, Debug, PartialEq,Default)]
#[derive(alkahest::Schema)]
#[derive(Savefile)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl alkahest::Pack<Vector3> for Vector3 {
    #[inline]
    fn pack(self, offset: usize, output: &mut [u8]) -> (alkahest::Packed<Self>, usize) {
        Vector3Pack {
            x: self.x,
            y: self.y,
            z: self.z,
        }
            .pack(offset, output)
    }
}

#[derive(Clone, Copy, Debug, PartialEq,Default)]
#[derive(alkahest::Schema)]
#[derive(Savefile)]
pub struct Triangle {
    pub v0: Vector3,
    pub v1: Vector3,
    pub v2: Vector3,
    pub normal: Vector3,
}
impl alkahest::Pack<Triangle> for &'_ Triangle {
    #[inline]
    fn pack(self, offset: usize, output: &mut [u8]) -> (alkahest::Packed<Triangle>, usize) {
        TrianglePack {
            v0: self.v0,
            v1: self.v1,
            v2: self.v2,
            normal: self.normal,
        }
            .pack(offset, output)
    }
}
#[derive(Clone, Debug, PartialEq,Default)]
#[derive(Savefile)]
pub struct Mesh {
    pub triangles: Vec<Triangle>,
}
#[derive(alkahest::Schema)]
pub struct MeshSchema {
    pub triangles: alkahest::Seq<Triangle>,
}

impl alkahest::Pack<MeshSchema> for &'_ Mesh {
    #[inline]
    fn pack(self, offset: usize, output: &mut [u8]) -> (alkahest::Packed<MeshSchema>, usize) {
        MeshSchemaPack {
            triangles: self.triangles.iter(),
        }
            .pack(offset, output)
    }
}


pub fn generate_mesh() -> Mesh {

    let mut mesh = Mesh {
        triangles: vec![]
    };
    const TRIANGLES: usize = 125_000;
    for _ in 0..TRIANGLES {
        mesh.triangles.push(Triangle::default())
    }

    mesh
}
#[bench]
fn bench_alkahest(b: &mut Bencher) {
    const BUFFER_LEN: usize = 10_000_000;
    let mesh = generate_mesh();
    let mut buffer = vec![0; BUFFER_LEN];
    b.iter(move || {
        alkahest::write::<MeshSchema, _>(black_box(&mut buffer), black_box(&mesh));
    });
}


#[bench]
fn bench_savefile(b: &mut Bencher) {
    let mesh = generate_mesh();
    let mut encoded: Vec<u8> = Vec::with_capacity(10_000_000);
    assert!(unsafe { Triangle::repr_c_optimization_safe(0).is_yes() } );
    b.iter(move || {
        /*let l = mesh.triangles.len();
        let data_ptr = mesh.triangles.as_ptr() as *const u8;
        let data_len = l * std::mem::size_of::<Triangle>();
        encoded
        serializer.write_buf(std::slice::from_raw_parts(
            self.as_ptr() as *const u8,
            std::mem::size_of::<T>() * l,
        ))*/
        encoded.clear();

        savefile::save_noschema(black_box(&mut encoded), 0, black_box(&mesh)).unwrap();
    })
}

/*




fn stuff<T>() -> T {
    todo!()
}
*/
#[test]
fn dummy() {
}