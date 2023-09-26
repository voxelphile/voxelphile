use std::collections::HashSet;

use nalgebra::SVector;
use serde_derive::{Deserialize, Serialize};

use super::{block::Block, ChunkPosition, PrecisePosition};
use crate::input::Input;
use band::Entity;

pub struct Hitbox {
    pub offset: SVector<f32, 3>,
    pub size: SVector<f32, 3>,
}

pub struct ChunkTranslation(pub ChunkPosition);

#[derive(Default)]
pub struct Translation(pub SVector<f32, 3>);
#[derive(Default)]
pub struct Look(pub SVector<f32, 2>);
#[derive(Default)]
pub struct Speed(pub f32);

#[derive(Default)]
pub struct Observer {
    pub view_distance: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Target(pub PrecisePosition);

pub struct Loader {
    pub load_distance: usize,
    pub last_translation_f: SVector<f32, 3>,
    pub recalculate_needed_chunks: bool,
    pub chunk_needed_iter: Box<dyn Iterator<Item = usize> + Send + Sync>,
}

#[derive(Default)]
pub struct Main;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum Change {
    Place(Block),
    Break(Block),
}
pub struct Break(pub Block);

pub struct Dirty(pub HashSet<Entity>);
pub struct Display;

pub type Degrees = f32;
pub type Power = f32;
pub type Resistance = f32;

pub enum Temperature {
    Ambient,
    Unique(Degrees),
}

pub struct Electric(pub Power);
