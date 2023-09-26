use super::structure::Chunk;
use super::{raycast, ChunkPosition, LocalPosition};
use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crate::input::Input;
use crate::net::{ChunkMessage, Client, ClientId, Server};
use crate::world;
use crate::world::block::*;
use crate::world::entity::{Break, Change};
use crate::world::structure::*;
use band::{QueryExt, *};
use crossbeam::channel::{self, Receiver, Sender};
use nalgebra::{SVector, Unit, UnitQuaternion};
use std::collections::VecDeque;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
    time::Duration,
};
use std::{ops, thread};
use strum::IntoEnumIterator;

use super::entity::{ChunkTranslation, Dirty, Display, Loader, Look, Observer, Speed, Translation};
use super::raycast::Ray;
use super::structure::{
    calc_block_visible_mask_between_chunks,
    calc_block_visible_mask_inside_chunk, gen, gen_block_mesh, neighbors, ClientBlockInfo,
    CHUNK_AXIS,
};

pub struct Dimension<C: Chunk> {
    chunks: HashMap<ChunkPosition, ChunkState<C>>,
    blocks_buffer: HashSet<(ChunkPosition, LocalPosition, Block)>,
    chunk_updated: HashSet<ChunkPosition>,
    chunk_border_changed: HashMap<ChunkPosition, HashSet<Direction>>,
    chunk_activations: HashSet<ChunkPosition>,
}

pub enum ChunkState<C: Chunk> {
    Generating,
    Stasis { neighbors: u8, chunk: C },
    Active { chunk: C },
}

pub fn raycast<'a>(
    this: &mut Dimension<ServerChunk>,
    target: raycast::Target,
    translation: SVector<f32, 3>,
    look: SVector<f32, 2>,
) -> Option<(ChunkPosition, LocalPosition)> {
    let forward_4d = (UnitQuaternion::from_axis_angle(
        &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
        look.x,
    ) * UnitQuaternion::from_axis_angle(
        &Unit::new_normalize(SVector::<f32, 3>::new(1.0, 0.0, 0.0)),
        look.y,
    ))
    .to_homogeneous()
        * SVector::<f32, 4>::new(0.0, 0.0, -1.0, 0.0);
    let direction = SVector::<f32, 3>::new(forward_4d.x, forward_4d.y, forward_4d.z);
    let origin = translation;

    let descriptor = raycast::Descriptor {
        origin,
        direction,
        minimum: SVector::<isize, 3>::new(isize::MIN, isize::MIN, isize::MIN),
        maximum: SVector::<isize, 3>::new(isize::MAX, isize::MAX, isize::MAX),
        max_distance: 10.0,
        chunks: &this.chunks,
    };

    let mut ray = raycast::start(descriptor);

    while let raycast::State::Traversal { .. } = raycast::drive(&mut ray).state {}

    let Some(raycast::Hit { position, back_step, .. }) = raycast::hit(ray) else {
    None?
};

    let target_position = if matches!(target, raycast::Target::Position) {
        position
    } else {
        back_step
    };

    let chunk_position = SVector::<isize, 3>::new(
        target_position.x.div_euclid(CHUNK_AXIS as _) as isize,
        target_position.y.div_euclid(CHUNK_AXIS as _) as isize,
        target_position.z.div_euclid(CHUNK_AXIS as _) as isize,
    );

    let chunk = match this.chunks.get(&chunk_position) {
        Some(ChunkState::Stasis { chunk, .. }) => chunk,
        Some(ChunkState::Active { chunk }) => chunk,
        _ => None?,
    };
    let mut position2 = target_position;
    let axis = chunk_axis(0);

    let local_position = SVector::<usize, 3>::new(
        target_position.x.rem_euclid(axis.x as _) as usize,
        target_position.y.rem_euclid(axis.y as _) as usize,
        target_position.z.rem_euclid(axis.z as _) as usize,
    );

    Some((chunk_position, local_position))
}

impl<C: Chunk> Dimension<C> {
    pub fn new() -> Self {
        Self {
            chunks: Default::default(),
            blocks_buffer: Default::default(),
            chunk_updated: Default::default(),
            chunk_border_changed: Default::default(),
            chunk_activations: Default::default(),
        }
    }

    pub fn get_chunks(&self) -> &HashMap<ChunkPosition, ChunkState<C>> {
        &self.chunks
    }

    pub fn get_chunks_mut(&mut self) -> &mut HashMap<ChunkPosition, ChunkState<C>> {
        &mut self.chunks
    }

    pub fn get_chunk_border_change_mut(
        &mut self,
    ) -> &mut HashMap<ChunkPosition, HashSet<Direction>> {
        &mut self.chunk_border_changed
    }

    pub fn get_chunk_updated_mut(&mut self) -> &mut HashSet<ChunkPosition> {
        &mut self.chunk_updated
    }

    pub fn get_chunk_activations_mut(&mut self) -> &mut HashSet<ChunkPosition> {
        &mut self.chunk_activations
    }
    pub fn get_chunk_updated(&self) -> &HashSet<ChunkPosition> {
        &self.chunk_updated
    }

    pub fn get_chunk_activations(&self) -> &HashSet<ChunkPosition> {
        &self.chunk_activations
    }
    /*
    
    pub fn flush_set_blocks(&mut self) {
        let set = self.blocks_buffer.drain().collect::<Vec<_>>();
        self.set_blocks(&set);
    }*/
    pub fn set_blocks(&mut self, blocks: &[(ChunkPosition, LocalPosition, Block)]) {
        let mut modified_chunks = HashSet::<ChunkPosition>::new();
        let mut set_block_in_chunk = HashMap::<ChunkPosition, Vec<(LocalPosition, Block)>>::new();

        for (chunk_position, local_position, block) in blocks {
            set_block_in_chunk
                .entry(*chunk_position)
                .or_default()
                .push((*local_position, *block));
        }
        for (chunk_position, blocks_at_position) in set_block_in_chunk {
            let chunk = match self.chunks.get_mut(&chunk_position) {
                Some(ChunkState::Active { chunk }) => chunk,
                Some(ChunkState::Stasis { chunk, .. }) => chunk,
                _ => {
                    let blocks_set_iter = blocks_at_position
                        .iter()
                        .copied()
                        .map(|(l, b)| (chunk_position, l, b));
                    self.blocks_buffer.extend(blocks_set_iter);
                    continue;
                }
            };
            for (local_position, block) in blocks_at_position {
                *chunk.get_mut(linearize(
                    chunk_axis(chunk.lod_level()),
                    local_position,
                )).block_mut() = block;
            }
            modified_chunks.insert(chunk_position);
        }
        self.chunk_updated.extend(modified_chunks);
    }
}