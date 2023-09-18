pub mod vertex;
use band::Registry;
use nalgebra::{SMatrix, SVector};
use vertex::*;
use winit::window::Window;

use crate::world::{block::Block, structure::{Direction}};

//#[cfg(feature = "boson")]
mod boson;
//#[cfg(feature = "boson")]
pub type Graphics = boson::Boson;

pub trait GraphicsInterface {
    fn init(window: &Window) -> Self;
    fn resize(&mut self, width: u32, height: u32);
    fn render(&mut self, registry: &mut Registry);
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bucket(pub(crate) usize);

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Indirect(pub usize);

pub struct BlockMesh {
    pub vertices: Vec<BlockVertex>,
    pub indices: Vec<u32>,
    pub position: SVector<f32, 3>,
}

pub struct Mesh {
    vertices: Vec<Bucket>,
    indices: Vec<Bucket>,
    indirect: Vec<Indirect>,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Camera {
    proj: SMatrix<f32, 4, 4>,
    view: SMatrix<f32, 4, 4>,
    trans: SMatrix<f32, 4, 4>,
    resolution: SVector<u32, 4>,
}
