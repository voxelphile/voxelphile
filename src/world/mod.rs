pub mod entity;
mod raycast;
pub mod structure;
pub mod block;

use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crate::input::Input;
use crate::world::entity::Break;
use crate::world::structure::Direction;
use band::{QueryExt, *};
use crossbeam::channel::{self, Receiver, Sender};
use nalgebra::{SVector, Unit, UnitQuaternion};
use strum::IntoEnumIterator;
use std::collections::VecDeque;
use std::thread;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
    time::Duration,
};
use structure::Chunk;

use self::entity::{Loader, Look, Observer, Place, Speed, Translation, Dirty};
use self::raycast::Ray;
use self::structure::{
    calc_block_visible_mask_between_chunks, calc_block_visible_mask_inside_structure,
    gen_block_mesh, gen_chunk, neighbors, BlockInfo, Structure, CHUNK_AXIS, CHUNK_SIZE,
};

pub struct Neighbors(u8);
pub struct Active;
pub struct Stasis;
pub struct Generating;
pub struct RemoveChunkMesh;
pub type ChunkPosition = SVector<isize, 3>;
pub type LocalPosition = SVector<usize, 3>;
pub type WorldPosition = SVector<isize, 3>;

pub struct World {
    chunks: HashMap<ChunkPosition, Entity>,
    loaded: HashSet<ChunkPosition>,
    pre_generator: (Sender<GenReq>, Receiver<GenReq>),
    post_generator: (Sender<GenResp>, Receiver<GenResp>),
    _generators: Vec<JoinHandle<()>>,
    load_chunk_order: VecDeque<ChunkPosition>,
}

impl World {
    pub fn new() -> Self {
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
            pre_generator: (pre_generator_tx, pre_generator_rx),
            post_generator: (post_generator_tx, post_generator_rx),
            _generators,
            chunks: Default::default(),
            loaded: Default::default(),
            load_chunk_order: VecDeque::new(),
        }
    }

    pub fn tick(&mut self, registry: &mut Registry, delta_time: f32) {
        for (Look(look), input) in <(&mut Look, &Input)>::query(registry) {
            *look += input.gaze;
        }
        for (Translation(translation), Look(look), Speed(speed), input) in
            <(&mut Translation, &Look, &Speed, &Input)>::query(registry)
        {
            *translation += speed
                * delta_time
                * (UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
                    look.x,
                )
                .to_rotation_matrix()
                    * input.direction);
        }
        /*{
            enum RaycastTarget {
                Position,
                Backstep,
            }
            let raycast = |world: &World,
                           target: RaycastTarget,
                           translation: SVector<f32, 3>,
                           look: SVector<f32, 2>|
             -> Option<(SVector<isize, 3>, SVector<usize, 3>)> {
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
                };

                let mut ray = raycast::start(descriptor);

                while let raycast::State::Traversal { .. } = raycast::drive(world, &mut ray).state {
                }

                let Some(raycast::Hit { position, back_step, .. }) = raycast::hit(ray) else {
                    None?
                };

                let target_position = if matches!(target, RaycastTarget::Position) {
                    position
                } else {
                    back_step
                };

                let chunk_position = SVector::<isize, 3>::new(
                    target_position.x.div_euclid(CHUNK_AXIS as isize),
                    target_position.y.div_euclid(CHUNK_AXIS as isize),
                    target_position.z.div_euclid(CHUNK_AXIS as isize),
                );
                let local_position = SVector::<usize, 3>::new(
                    target_position.x.rem_euclid(CHUNK_AXIS as isize) as usize,
                    target_position.y.rem_euclid(CHUNK_AXIS as isize) as usize,
                    target_position.z.rem_euclid(CHUNK_AXIS as isize) as usize,
                );
                Some((chunk_position, local_position))
            };
            let mut modified_chunks = HashSet::<SVector<isize, 3>>::new();
            let mut remove_place = HashSet::<Entity>::new();
            let mut remove_break = HashSet::<Entity>::new();
            for (e, Translation(translation), Look(look), Place(block)) in
                <(Entity, &Translation, &Look, &Place)>::query(&mut registry)
            {
                remove_place.insert(e);

                let Some((chunk_position, local_position)) = (raycast)(self, RaycastTarget::Backstep, *translation, *look) else {
                    continue
                };

                let chunk = match self.get_chunk_state(registry, chunk_position) {
                    Some(ChunkState::Active { chunk, .. }) => chunk,
                    _ => continue,
                };
                chunk.get_mut(chunk.linearize(local_position)).block = *block;
                modified_chunks.insert(chunk_position);
            }
            for (e, Translation(translation), Look(look), Break(block)) in
                <(Entity, &Translation, &Look, &Break)>::query(&mut registry)
            {
                remove_break.insert(e);

                let Some((chunk_position, local_position)) = (raycast)(self, RaycastTarget::Position, *translation, *look) else {
                    continue
                };

                let chunk = match self.get_chunk_state(registry, chunk_position) {
                    Some(ChunkState::Active { chunk, .. }) => chunk,
                    _ => continue,
                };
                chunk.get_mut(chunk.linearize(local_position)).block = *block;
                modified_chunks.insert(chunk_position);
            }
            let mut update_mesh = HashSet::new();
            for position in modified_chunks {
                let Some(mut chunk) = self.get_chunk(registry, position, |s| matches!(s, ChunkState::Active { .. })) else {
                    continue;
                };
                for i in 0..CHUNK_SIZE {
                    let mask_overlay = calc_block_visible_mask_inside_structure(chunk, i);
                        chunk.get_mut(i).visible_mask = mask_overlay;
                }
                update_mesh.insert(position);
                neighbors(position, |neighbor, dir, dimension, normal| {
                    let (mut neighbor_chunk, active) = match self.get_chunk_state(registry, position) {
                        Some(ChunkState::Stasis { chunk, .. }) => (chunk, false),
                        Some(ChunkState::Active { chunk, .. }) => (chunk, true),
                        _ => return,
                    };

                    let changed = calc_block_visible_mask_between_chunks(
                        &mut chunk,
                        &mut neighbor_chunk,
                        dir,
                        dimension,
                        normal,
                    );
                    if changed && active {
                        update_mesh.insert(neighbor);
                    }
                });
            }
            for position in update_mesh {
                use ChunkState::*; 
                match self.get_chunk_state(registry, position) {
                    Some(Active { dirty, .. }) => *dirty = true,
                    _ => {}
                };
            }
            for e in remove_place {
                registry.remove::<Place>(e);
            }
            for e in remove_break {
                registry.remove::<Break>(e);
            } }*/
    }

    pub fn display(&mut self, registry: &mut Registry) {
        let Some((Translation(translation_f), _)) = <(&Translation, &Observer)>::query(registry).next() else {
            return;
        };
        let mut activate = HashSet::new();
        for (entity, neighbors, _, _) in <(Entity, &Neighbors, &Chunk, &Stasis)>::query(registry) {
            if neighbors.0 == 6 {
                activate.insert(entity);
            }
        }
        for entity in activate {
            registry.remove::<Stasis>(entity);
            registry.insert(entity, Active);
            registry.insert(entity, Dirty);
        }
        /*let mut activate = HashSet::new();
        let mut iter = self
            .chunks
            .iter()
            .map(|(p, e)| (p, registry.get(*e).unwrap()))
            .filter(|(_, x)| matches!(x, ChunkState::Stasis { .. }));

        while let Some((position, ChunkState::Stasis { neighbors, .. })) = iter.next() {
            if *neighbors == 6 {
                activate.insert(*position);
            }
        }
        for position in activate {
            let position_f = nalgebra::convert::<_, SVector<f32, 3>>(position) * CHUNK_AXIS as f32;
            if position_f.metric_distance(&translation_f)
                > self.view_distance as f32 * CHUNK_AXIS as f32
            {
                continue;
            }
            /*
            let Some(ChunkState::Stasis { chunk, .. }) = self.chunks.remove(&position) else {
                    continue;
                };
            let (vertices, indices) =
                gen_block_mesh(&chunk, |block, dir| graphics.block_mapping(block, dir));
            let mesh = graphics.create_block_mesh(BlockMesh {
                vertices: &vertices,
                indices: &indices,
                position: position_f,
            });

            self.chunks
                .insert(position, ChunkState::Active { chunk, mesh });*/
            let Some(ChunkState::Stasis { chunk, .. }) = registry.remove::<ChunkState>(self.chunks[&position]) else {
                continue;
            };

            registry.insert(self.chunks[&position], ChunkState::Active { chunk, dirty: true });
        }*/
    }
    pub fn load(&mut self, registry: &mut Registry) {
        for (
            Translation(translation_f),
            Loader {
                load_distance,
                chunk_needed_iter,
                recalculate_needed_chunks,
                last_translation_f,
            },
        ) in <(&Translation, &mut Loader)>::query(registry)
        {
            let translation =
                nalgebra::try_convert::<_, SVector<isize, 3>>(*translation_f).unwrap();

            let chunk_translation = translation / CHUNK_AXIS as isize;

            if last_translation_f.metric_distance(&translation_f) >= *load_distance as f32 / 4.0
            {
                *last_translation_f = *translation_f;
                let side_length = 2 * *load_distance + 1;
                let total_chunks = side_length * side_length * side_length;
                *chunk_needed_iter = Box::new(0..total_chunks);
                *recalculate_needed_chunks = true;
            }

            if *recalculate_needed_chunks {
                let mut a = 0;
                loop {
                    let Some(i) = chunk_needed_iter.next() else {
                    *recalculate_needed_chunks = false;
                    break;
                };
                    let mut b = 0;
                    let mut pos = SVector::<isize, 3>::new(0, 0, 0);
                    'a: for j in 0..*load_distance as isize {
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
            let (pre_generator_tx, _) = &self.pre_generator;

            let _ = pre_generator_tx.send(GenReq { position });

            let entity = registry.spawn();
            {
                registry.insert(entity, Translation(nalgebra::convert::<_, SVector<f32, 3>>(position) * CHUNK_AXIS as f32));
                registry.insert(entity, Chunk::default());
                registry.insert(entity, Neighbors(0));
                registry.insert(entity, Generating);
            }
            self.chunks.insert(position, entity);
            self.loaded.insert(position);
        }

        {
            let (_, post_generator_rx) = &self.post_generator;
            while let Ok(GenResp {
                position,
                chunk: mut my_chunk,
            }) = post_generator_rx.try_recv()
            {
                let entity = self.chunks[&position];

                let mut neighbors_present = 0;
                neighbors(position, |neighbor, dir, dimension, normal| {
                    let Some(neighbor_entity) = self.chunks.get(&neighbor) else {
                        return;
                    };

                    if registry.get::<Generating>(*neighbor_entity).is_some() {
                        return;
                    }

                    if registry.get::<Stasis>(*neighbor_entity).is_some() {
                        registry.get_mut::<Neighbors>(*neighbor_entity).unwrap().0 += 1;
                    }

                    neighbors_present += 1;

                    let their_chunk = registry.get_mut::<Chunk>(*neighbor_entity).unwrap();

                    calc_block_visible_mask_between_chunks(
                        &mut my_chunk,
                        their_chunk,
                        dir,
                        dimension,
                        normal,
                    );
                });

                *registry.get_mut::<Chunk>(entity).unwrap() = my_chunk;
                registry.get_mut::<Neighbors>(entity).unwrap().0 = neighbors_present;
                registry.remove::<Generating>(entity);
                registry.insert(entity, Stasis);
            }
        }

        /*for position in self.load_chunk_order.drain(..) {
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
        }*/
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
