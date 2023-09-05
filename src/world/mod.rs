pub mod structure;

use nalgebra::SVector;
use std::{collections::{HashMap, HashSet}, hash::Hash};
use structure::Chunk;

use crate::graphics::{Mesh, Graphics, GraphicsInterface, BlockMesh};

use self::structure::{CHUNK_AXIS, gen_chunk, gen_block_mesh};

pub enum ChunkState {
    Active {
        chunk: Chunk,
        mesh: Mesh,
    }
}

#[derive(Default)]
pub struct World {
    chunks: HashMap<SVector<isize, 3>, ChunkState>,
    loaded: HashSet<SVector<isize, 3>>,
    view_distance: usize,
}

impl World {
    pub fn new(view_distance: usize) -> Self {
        Self {
            view_distance,
            ..Default::default()
        }
    }

    pub fn load(&mut self, graphics: &mut Graphics, translation: SVector<f32, 3>) {
        let translation = nalgebra::try_convert::<_, SVector<isize, 3>>(translation).unwrap();

        let view_distance =self.view_distance as isize;

        let mut needed_chunks = HashSet::default();

        for x in -view_distance..=view_distance {
            for y in -view_distance..=view_distance {
                for z in -view_distance..=view_distance {
                    let position = SVector::<isize, 3>::new(x, y, z);
                    needed_chunks.insert(translation / CHUNK_AXIS as isize + position);

                }
            }
        }

        for position in needed_chunks.difference(&self.loaded).cloned().collect::<Vec<_>>() {
            let chunk = gen_chunk();
            let (vertices, indices) = gen_block_mesh(&chunk);
            let mesh = graphics.create_block_mesh(BlockMesh {
                vertices: &vertices,
                indices: &indices,
                position: nalgebra::convert::<_, SVector<f32, 3>>(position) * CHUNK_AXIS as f32,
            });
            
            self.chunks.insert(position, ChunkState::Active { chunk, mesh });
            self.loaded.insert(position);

            
        }
    }
}