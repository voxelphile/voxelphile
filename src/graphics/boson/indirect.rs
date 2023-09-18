use boson::prelude::DrawIndirectCommand;
use nalgebra::base::*;

use super::buffer::indirect::IndirectProvider;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlockIndirectData {
    pub cmd: DrawIndirectCommand,
    pub position: SVector<f32, 4>,
}

impl IndirectProvider for BlockIndirectData {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EntityIndirectData {
    pub cmd: DrawIndirectCommand,
    pub position: SVector<f32, 4>,
    pub rotation: SVector<f32, 4>,
    pub scale: SVector<f32, 4>,
}

impl IndirectProvider for EntityIndirectData {}
