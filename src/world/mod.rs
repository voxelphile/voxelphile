pub mod entity;
pub mod structure;

use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crate::input::Input;
use crossbeam::channel::{self, Receiver, Sender};
use nalgebra::{SVector, Unit, UnitQuaternion};
use std::collections::VecDeque;
use std::thread;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
    time::Duration,
};
use structure::Chunk;

use self::entity::Entity;
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
    entity_cursor: usize,
    observer_entity: usize,
    loader_entities: HashSet<usize>,
    entities: HashMap<usize, Entity>,
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
            entity_cursor: 0,
            observer_entity: 0,
            loader_entities: HashSet::new(),
            entities: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, entity: Entity) -> usize {
        let id = self.entity_cursor;
        self.entities.insert(id, entity);
        self.entity_cursor += 1;
        id
    }

    pub fn supply_observer_input(&mut self, input: Input) {
        use Entity::*;
        *match self.entities.get_mut(&self.observer_entity).unwrap() {
            Player {input,..} => input
        } = input;
    }

    pub fn get_observer(&self) -> &Entity {
        &self.entities[&self.observer_entity]
    }

    pub fn set_observer(&mut self, id: usize) {
        self.observer_entity = id;
        self.loader_entities.insert(id);
    }

    pub fn tick(&mut self, delta_time: f32) {
        for (id, entity) in &mut self.entities {
            use Entity::*;
            match entity {
                Player { translation, look, input, speed } => {
                    movement(delta_time, translation, look, input, *speed);
                }
            }
        }
        

    }

    pub fn display(&mut self, graphics: &mut Graphics) {
        let observer_entity = &self.entities[&self.observer_entity];
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
            
            use Entity::*;
            let translation_f = match observer_entity {
                Player { translation, .. } => translation
            };

            let position_f = nalgebra::convert::<_, SVector<f32, 3>>(position) * CHUNK_AXIS as f32;
            if position_f.metric_distance(&translation_f)
                > self.view_distance as f32 * CHUNK_AXIS as f32
            {
                continue;
            }
            let Some(ChunkState::Stasis { chunk, .. }) = self.chunks.remove(&position) else {
                    continue;
                };
            let (vertices, indices) = gen_block_mesh(&chunk, |block| graphics.block_mapping(block));
            let mesh = graphics.create_block_mesh(BlockMesh {
                vertices: &vertices,
                indices: &indices,
                position: position_f,
            });

            self.chunks
                .insert(position, ChunkState::Active { chunk, mesh });
        }
    }
    pub fn load(&mut self) {
        for entity_id in &self.loader_entities {
            let entity = &self.entities[entity_id];

            use Entity::*;
            let translation_f = match entity {
                Player { translation, .. } => translation
            };

            let translation =
                nalgebra::try_convert::<_, SVector<isize, 3>>(*translation_f).unwrap();

            let chunk_translation = translation / CHUNK_AXIS as isize;

            if self.last_translation.metric_distance(&translation_f)
                >= self.view_distance as f32 / 4.0
            {
                self.last_translation = *translation_f;
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
                    let pos = pos + chunk_translation;
                    if !self.loaded.contains(&pos) {
                        self.load_chunk_order.push_back(pos);
                    }
                    a += 1;
                    if a >= 10 {
                        break;
                    }
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
    }
}

fn movement(delta_time: f32, translation: &mut SVector<f32, 3>, look: &mut SVector<f32, 2>, input: &Input, speed: f32) {
        *look += input.gaze;
        *translation += speed * delta_time
        * (UnitQuaternion::from_axis_angle(
            &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
            look.x,
        )
        .to_rotation_matrix()
            * input.direction);
    
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
