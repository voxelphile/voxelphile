pub mod vertex;
use nalgebra::{SMatrix, SVector};
use vertex::*;
use winit::window::Window;

//#[cfg(feature = "boson")]
mod boson;
//#[cfg(feature = "boson")]
pub type Graphics = boson::Boson;

pub trait GraphicsInterface {
    fn init(window: &Window) -> Self;
    fn create_block_mesh(&mut self, info: BlockMesh<'_>) -> Mesh;
    fn resize(&mut self, width: u32, height: u32);
    fn render(&mut self, look: SVector<f32, 2>, translation: SVector<f32, 3>);
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bucket(pub(crate) usize);

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Indirect(pub usize);

pub struct BlockMesh<'a> {
    pub vertices: &'a [BlockVertex],
    pub indices: &'a [u32],
    pub position: SVector<f32, 3>,
}

pub struct Mesh {
    vertices: Vec<Bucket>,
    indices: Vec<Bucket>,
    indirect: Vec<Indirect>,
}

#[derive(Clone, Copy)]
pub struct Camera {
    proj: SMatrix<f32, 4, 4>,
    view: SMatrix<f32, 4, 4>,
}
