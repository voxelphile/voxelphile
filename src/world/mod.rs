pub mod structure;

use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crossbeam::channel::{self, Receiver, Sender};
use nalgebra::SVector;
use std::collections::VecDeque;
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
    Structure, CHUNK_AXIS, CHUNK_SIZE,
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
    last_translation: SVector<f32, 3>,
    load_chunk_order: VecDeque<SVector<isize, 3>>,
    recalculate_needed_chunks: bool,
    chunk_needed_iter: Box<dyn Iterator<Item = usize>>,
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

        Self {
            view_distance,
            pre_generator: (pre_generator_tx, pre_generator_rx),
            post_generator: (post_generator_tx, post_generator_rx),
            _generators,
            chunks: Default::default(),
            loaded: Default::default(),
            last_translation: Default::default(),
            load_chunk_order: VecDeque::new(),
            recalculate_needed_chunks: false,
            chunk_needed_iter: Box::new(0..0),
        }
    }

    pub fn load(&mut self, graphics: &mut Graphics, translation_f: SVector<f32, 3>) {
        let translation = nalgebra::try_convert::<_, SVector<isize, 3>>(translation_f).unwrap();

        let chunk_translation = translation / CHUNK_AXIS as isize;

        if self.last_translation.metric_distance(&translation_f) >= self.view_distance as f32 / 4.0 
        {
            self.last_translation = translation_f;
            let side_length = 2 * self.view_distance + 1;
            let total_chunks = side_length * side_length * side_length;
            self.chunk_needed_iter = Box::new(0..total_chunks);
            self.load_chunk_order = VecDeque::with_capacity(total_chunks);
            self.recalculate_needed_chunks = true;
        }

        if self.recalculate_needed_chunks {
            let mut a = 0;
            loop {
                let Some(i) = self.chunk_needed_iter.next() else {
                self.recalculate_needed_chunks = false;
                break;
            };
                let mut b = 0;
                let mut pos = SVector::<isize, 3>::new(0, 0, 0);
                'a: for j in 0..self.view_distance as isize {
                    for x in -j..=j {
                        for y in -j..=j {
                            for z in -j..=j {
                                if x.abs() != j && y.abs() != j && z.abs() != j {
                                    continue;
                                }
                                if i == b {
                                    pos = SVector::<isize, 3>::new(x, y, z);
                                    break 'a;
                                }
                                b += 1;
                            }
                        }
                    }
                }
                self.load_chunk_order
                    .push_back(pos + chunk_translation);
                dbg!(i);
                a += 1;
                if a >= 10 {
                    break;
                }
            }
        }

        for position in self.load_chunk_order.drain(..) {
            if !self.loaded.contains(&position) {
                let (pre_generator_tx, _) = &self.pre_generator;

                let _ = pre_generator_tx.send(GenReq { position });

                self.chunks.insert(position, ChunkState::Generating);
                self.loaded.insert(position);
            }
        }

        {
            let (_, post_generator_rx) = &self.post_generator;
            while let Ok(GenResp {
                position,
                mut chunk,
            }) = post_generator_rx.try_recv()
            {
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
