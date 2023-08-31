use boson::prelude::DrawIndirectCommand;
use nalgebra::base::*;

use super::buffer::indirect::IndirectProvider;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IndirectData {
    pub cmd: DrawIndirectCommand,
    pub position: SVector<f32, 4>,
}

impl IndirectProvider for IndirectData {}
