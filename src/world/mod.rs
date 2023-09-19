pub mod block;
pub mod entity;
mod raycast;
pub mod structure;

use self::block::*;
use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crate::input::Input;
use crate::world;
use crate::world::entity::{Break, Change};
use crate::world::structure::{calc_ambient_inside_chunk, Direction};
use band::{QueryExt, *};
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
use strum::IntoEnumIterator;

use self::entity::{Dirty, Loader, Look, Observer, Speed, Translation};
use self::raycast::Ray;
use self::structure::{
    calc_and_set_ambient_between_chunk_neighbors, calc_block_visible_mask_between_chunks,
    calc_block_visible_mask_inside_structure, gen_block_mesh, gen_chunk, neighbors, BlockInfo,
    Structure, CHUNK_AXIS, CHUNK_SIZE,
};

pub struct Neighbors(u8);
pub struct Active;
pub struct Stasis;
pub struct Generating;
pub struct RemoveChunkMesh;
pub struct NeedsAoBorderCalc;
pub struct Lod(pub u8);
pub struct Unique;
pub type ChunkPosition = SVector<isize, 3>;
pub type LocalPosition = SVector<usize, 3>;
pub type WorldPosition = SVector<isize, 3>;

pub struct World {
    chunks: HashMap<ChunkPosition, Entity>,
    mapping: HashMap<Entity, ChunkPosition>,
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
            mapping: Default::default(),
            loaded: Default::default(),
            load_chunk_order: VecDeque::new(),
        }
    }

    pub fn set_blocks(&mut self, registry: &mut Registry, blocks: &[(WorldPosition, Block)]) {
        let mut modified_chunks = HashSet::<SVector<isize, 3>>::new();
        let mut set_block_in_chunk = HashMap::<ChunkPosition, Vec<(LocalPosition, Block)>>::new();
        for (target_position, block) in blocks {
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

            set_block_in_chunk.entry(chunk_position).or_default().push((local_position, *block));
        }
        for (chunk_position, blocks_at_position) in set_block_in_chunk {
            let chunk = registry.get_mut::<Chunk>(self.chunks[&chunk_position]).unwrap();
            for (local_position, block) in blocks_at_position {
                chunk.get_mut(chunk.linearize(local_position)).block = block;
            }
            modified_chunks.insert(chunk_position);
        }
        let mut update_mesh = HashSet::new();
        let mut needs_ao_recalc = HashSet::<SVector<isize, 3>>::new();
        for position in modified_chunks {
            let mut chunk = registry.remove::<Chunk>(self.chunks[&position]).unwrap();
            for i in 0..CHUNK_SIZE {
                let mask = calc_block_visible_mask_inside_structure(&chunk, i);
                let ambient = calc_ambient_inside_chunk(&chunk, i);
                chunk.get_mut(i).visible_mask = mask;
                chunk.get_mut(i).ambient = ambient;
            }
            update_mesh.insert(position);
            neighbors(position, |neighbor, dir, dimension, normal| {
                let neighbor_chunk = registry.get_mut::<Chunk>(self.chunks[&neighbor]).unwrap();

                let changed = calc_block_visible_mask_between_chunks(
                    &mut chunk,
                    neighbor_chunk,
                    dir,
                    dimension,
                    normal,
                );
                if changed {
                    needs_ao_recalc.insert(position);
                    needs_ao_recalc.insert(neighbor);
                    update_mesh.insert(neighbor);
                }
            });
            registry.insert::<Chunk>(self.chunks[&position], chunk);
        }
        for position in needs_ao_recalc {
            registry.insert(self.chunks[&position], NeedsAoBorderCalc);
        }
        for position in update_mesh {
            registry.insert(self.chunks[&position], Dirty);
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

        {
            enum RaycastTarget {
                Position,
                Backstep,
            }
            fn raycast<'a>(
                world: &'a World,
                registry: &'a Registry,
                target: RaycastTarget,
                translation: SVector<f32, 3>,
                look: SVector<f32, 2>,
            ) -> Option<(SVector<isize, 3>)> {
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
                    chunks: &world.chunks,
                    registry,
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

                Some(target_position)
            };
            let mut remove_change = HashSet::<Entity>::new();
            let mut block_changes = Vec::<(WorldPosition, Block)>::new();
            for (e, Translation(translation), Look(look), change) in
                <(Entity, &Translation, &Look, &Change)>::query(registry)
            {
                remove_change.insert(e);

                use Change::*;
                let target = match change {
                    Place(_) => RaycastTarget::Backstep,
                    Break(_) => RaycastTarget::Position,
                };
                let Some(world_position) = raycast(self, registry, target, *translation, *look) else {
                continue
            };

                block_changes.push((
                    world_position,
                    match change {
                        Place(b) => *b,
                        Break(b) => *b,
                    },
                ));
            }
            self.set_blocks(registry, &block_changes);
            for e in remove_change {
                registry.remove::<Change>(e);
            }
        }
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

            let mut unique = false;
            let chunk = registry.get::<Chunk>(entity).unwrap();
            for i in 0..CHUNK_SIZE {
                if chunk.get(i).visible_mask != 0xFF {
                    unique = true;
                    break;
                }
            }
            if !unique {
                continue;
            }
            registry.insert(entity, Dirty);
            registry.insert(
                entity,
                Lod(
                    (translation_f.metric_distance(&registry.get::<Translation>(entity).unwrap().0)
                        / 128.0) as u8,
                ),
            );
            registry.insert(entity, NeedsAoBorderCalc);
        }
        let mut remove_ao_calc = HashSet::new();
        'a: for (entity, _) in <(Entity, &NeedsAoBorderCalc)>::query(registry) {
            let position = self.mapping[&entity];
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let off = SVector::<isize, 3>::new(x, y, z);
                        let position = position + off;
                        if !self.loaded.contains(&position)
                            || registry.get::<Generating>(self.chunks[&position]).is_some()
                        {
                            continue 'a;
                        }
                    }
                }
            }

            calc_and_set_ambient_between_chunk_neighbors(registry, &self.chunks, position);
            remove_ao_calc.insert(entity);
        }
        for entity in remove_ao_calc {
            registry.remove::<NeedsAoBorderCalc>(entity);
        }
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

            if last_translation_f.metric_distance(&translation_f) >= *load_distance as f32 / 4.0 {
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
                registry.insert(
                    entity,
                    Translation(
                        nalgebra::convert::<_, SVector<f32, 3>>(position) * CHUNK_AXIS as f32,
                    ),
                );
                registry.insert(entity, Chunk::default());
                registry.insert(entity, Neighbors(0));
                registry.insert(entity, Generating);
            }
            self.chunks.insert(position, entity);
            self.mapping.insert(entity, position);
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
