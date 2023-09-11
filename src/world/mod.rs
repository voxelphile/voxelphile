pub mod structure;

use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crossbeam::channel::{self, Receiver, Sender};
use nalgebra::SVector;
use std::thread;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
    time::Duration,
};
use structure::Chunk;

use self::structure::{
    calc_block_visible_mask_between_chunks, gen_block_mesh, gen_chunk, neighbors, BlockInfo,
    Structure, CHUNK_AXIS,
};

pub enum ChunkState {
    Generating,
    Stasis { chunk: Chunk, neighbors: u8 },
    Active { chunk: Chunk, mesh: Mesh },
}

pub struct World {
    chunks: HashMap<SVector<isize, 3>, ChunkState>,
    loaded: HashSet<SVector<isize, 3>>,
    pre_generator: (Sender<GenReq>, Receiver<GenReq>),
    post_generator: (Sender<GenResp>, Receiver<GenResp>),
    _generators: Vec<JoinHandle<()>>,
    view_distance: usize,
    chunk_translation: SVector<isize, 3>,
    last_chunk_translation: SVector<isize, 3>,
    load_chunk_order: Vec<SVector<isize, 3>>,
}

impl World {
    pub fn new(view_distance: usize) -> Self {
        let (pre_generator_tx, pre_generator_rx) = channel::unbounded();
        let (post_generator_tx, post_generator_rx) = channel::unbounded();
        let _generators = (0..7)
            .into_iter()
            .map(|_| {
                let pre_generator_rx = pre_generator_rx.clone();
                let post_generator_tx = post_generator_tx.clone();
                thread::spawn(|| {
                    generator(post_generator_tx, pre_generator_rx);
                })
            })
            .collect::<Vec<_>>();
        let chunk_translation = SVector::<isize, 3>::new(0, 0, 0);
        let last_chunk_translation = SVector::<isize, 3>::new(isize::MAX, 0, 0);

        Self {
            view_distance,
            pre_generator: (pre_generator_tx, pre_generator_rx),
            post_generator: (post_generator_tx, post_generator_rx),
            _generators,
            chunks: Default::default(),
            loaded: Default::default(),
            chunk_translation,
            last_chunk_translation,
            load_chunk_order: vec![],
        }
    }

    pub fn load(&mut self, graphics: &mut Graphics, translation_f: SVector<f32, 3>) {
        let translation = nalgebra::try_convert::<_, SVector<isize, 3>>(translation_f).unwrap();

        let view_distance = self.view_distance as isize;

        self.chunk_translation = translation / CHUNK_AXIS as isize;

        if self.chunk_translation != self.last_chunk_translation {
            self.last_chunk_translation = self.chunk_translation;
            let mut needed_chunks = HashSet::default();
            for x in -view_distance..=view_distance {
                for y in -view_distance..=view_distance {
                    for z in -view_distance / 2..=view_distance / 2 {
                        let position = SVector::<isize, 3>::new(x, y, z);
                        needed_chunks.insert(translation / CHUNK_AXIS as isize + position);
                    }
                }
            }

            self.load_chunk_order = needed_chunks
                .difference(&self.loaded)
                .cloned()
                .collect::<Vec<_>>();

            self.load_chunk_order.sort_by(|a, b| {
                let a_f = nalgebra::convert::<_, SVector<f32, 3>>(*a);
                let b_f = nalgebra::convert::<_, SVector<f32, 3>>(*b);
                a_f.metric_distance(&translation_f)
                    .partial_cmp(&b_f.metric_distance(&translation_f))
                    .unwrap()
            });
        }

        for position in self
            .load_chunk_order
            .drain(..10)
        {
            let (pre_generator_tx, _) = &self.pre_generator;

            let _ = pre_generator_tx.send(GenReq { position });

            self.chunks.insert(position, ChunkState::Generating);
            self.loaded.insert(position);
        }

        {
            let (_, post_generator_rx) = &self.post_generator;
            while let Ok(GenResp {
                position,
                mut chunk,
            }) = post_generator_rx.try_recv()
            {
            dbg!(position);
            let mut neighbors_present = 0;
                neighbors(position, |neighbor, dir, dimension, normal| {
                    let mut neighbor = match self.chunks.get_mut(&neighbor) {
                        Some(ChunkState::Stasis { chunk, neighbors }) => {
                            *neighbors += 1;
                            chunk
                        }
                        Some(ChunkState::Active { chunk, .. }) => chunk,
                        _ => return,
                    };
                    neighbors_present += 1;
                    calc_block_visible_mask_between_chunks(
                        &mut chunk,
                        &mut neighbor,
                        dir,
                        dimension,
                        normal,
                    );
                });

                self.chunks.insert(
                    position,
                    ChunkState::Stasis {
                        chunk,
                        neighbors: neighbors_present,
                    },
                );
            }
        }

        {
            let mut activate = HashSet::new();
            let mut iter = self
                .chunks
                .iter()
                .filter(|(_, x)| matches!(x, ChunkState::Stasis { .. }));

            while let Some((position, ChunkState::Stasis { neighbors, .. })) = iter.next() {
                if *neighbors == 6 {
                    activate.insert(*position);
                }
            }
            for position in activate {
                let Some(ChunkState::Stasis { chunk, .. }) = self.chunks.remove(&position) else {
                    continue;
                };
                let (vertices, indices) = gen_block_mesh(&chunk);
                let mesh = graphics.create_block_mesh(BlockMesh {
                    vertices: &vertices,
                    indices: &indices,
                    position: nalgebra::convert::<_, SVector<f32, 3>>(position) * CHUNK_AXIS as f32,
                });

                self.chunks
                    .insert(position, ChunkState::Active { chunk, mesh });
            }
        }
    }
}

pub struct GenReq {
    position: SVector<isize, 3>,
}

pub struct GenResp {
    position: SVector<isize, 3>,
    chunk: Chunk,
}

pub fn generator(post_generator_tx: Sender<GenResp>, pre_generator_rx: Receiver<GenReq>) {
    loop {
        let Ok(GenReq { position }) = pre_generator_rx.try_recv() else {
            thread::sleep(Duration::from_millis(1));
            continue;
        };

        let chunk = gen_chunk(CHUNK_AXIS as isize * position);

        let _ = post_generator_tx.send(GenResp { position, chunk });
    }
}
