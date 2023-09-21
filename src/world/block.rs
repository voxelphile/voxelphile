use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::world::structure::Direction;

#[derive(EnumIter, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum Block {
    Air,
    Stone,
    Machine,
    Wire,
    Source,
}

impl Block {
    pub fn texture_name(&self, dir: Direction) -> Option<String> {
        use Block::*;
        use Direction::*;
        Some(String::from(match (self, dir) {
            (Stone, _) => "stone",
            (Machine, Forward) => "machine_front",
            (Machine, _) => "machine_side",
            (Wire, _) => "wire",
            (Source, _) => "source",
            //Wood => "wood",
            _ => None?,
        }))
    }
    pub fn parallax(&self) -> bool {
        use Block::*;
        match self {
            Stone => true,
            _ => false,
        }
    }
    pub fn normal(&self) -> bool {
        use Block::*;
        match self {
            Stone => true,
            _ => false,
        }
    }
    pub fn is_opaque(&self) -> bool {
        !matches!(self, Block::Air)
    }
}

pub mod entity {
    use nalgebra::SVector;

    pub struct Tile(SVector<isize, 3>);
}
