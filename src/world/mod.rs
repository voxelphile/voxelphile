pub mod block;
pub mod dimension;
pub mod entity;
pub mod raycast;
pub mod structure;

use self::block::*;
use self::dimension::{ChunkState, Dimension};
use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crate::input::Input;
use crate::net::{ChunkActivated, ChunkMessage, ChunkUpdated, Client, ClientId, Message, Server, ClientTag, ServerTag};
use crate::util::rle;
use crate::world;
use crate::world::entity::{Break, Change};
use crate::world::structure::{calc_ambient_inside_chunk, Direction};
use band::{QueryExt, *};
use crossbeam::channel::{self, Receiver, Sender};
use lerp::num_traits::ToBytes;
use nalgebra::{Dim, SVector, Unit, UnitQuaternion};
use std::collections::VecDeque;
use std::thread;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
    time::Duration,
};
use std::{mem, ops};
use structure::Chunk;
use strum::IntoEnumIterator;

use self::entity::{ChunkTranslation, Dirty, Display, Loader, Look, Observer, Speed, Translation};
use self::raycast::Ray;
use self::structure::{
    calc_and_set_ambient_between_chunk_neighbors, calc_block_visible_mask_between_chunks,
    calc_block_visible_mask_inside_chunk, gen, gen_block_mesh, neighbors, BlockInfo, CHUNK_AXIS,
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
pub type PrecisePosition = SVector<f32, 3>;

pub struct ChunkMarker;

pub const LOD_VIEW_FACTOR: f32 = 256.0;

pub struct DimensionState {
    chunk_entity_mapping: HashMap<ChunkPosition, Entity>,
    entity_chunk_mapping: HashMap<Entity, ChunkPosition>,
    dimension: Dimension,
}

impl ops::Deref for DimensionState {
    type Target = Dimension;
    fn deref(&self) -> &Self::Target {
        &self.dimension
    }
}

impl ops::DerefMut for DimensionState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dimension
    }
}

pub struct ClientWorld {
    dimension_state: DimensionState,
}

fn common_tick<Tag: Component>(registry: &mut Registry, delta_time: f32) {
    for (Look(look), input, _) in <(&mut Look, &Input, &Tag)>::query(registry) {
        *look += input.gaze;
    }
    for (Translation(translation), Look(look), Speed(speed), input, _) in
        <(&mut Translation, &Look, &Speed, &Input, &Tag)>::query(registry)
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
}

impl ClientWorld {
    pub fn new() -> Self {
        Self {
            dimension_state: DimensionState {
                dimension: Dimension::new(),
                chunk_entity_mapping: Default::default(),
                entity_chunk_mapping: Default::default(),
            },
        }
    }

    pub fn tick(&mut self, registry: &mut Registry, delta_time: f32) {
        let client = registry.resource_mut::<Client>().unwrap();
        let _ = client.recv();

        common_tick::<ClientTag>(registry, delta_time);
        
        if let Some((input, _)) = <(&Input, &Observer)>::query(registry).next() {
            client.send(Message::Input(input.clone())).unwrap();
        }
        let mut remove_change = None;
        if let Some((entity, change, _)) = <(Entity, &Change, &Observer)>::query(registry).next() {
            client.send(Message::Change(*change)).unwrap();
            remove_change = Some(entity);
        }
        if let Some(entity) = remove_change {
            registry.remove::<Change>(entity);
        }
        for message in client.get(|msg| matches!(msg, Message::Chunk(ChunkMessage::Activated(_)))) {
            let Message::Chunk(ChunkMessage::Activated(ChunkActivated { position, lod, mut bytes })) = message else {
                continue;
            };

            let mut chunk = Chunk::new(lod);

            /*let neighbors_present = 0;
            neighbors(position, |neighbor, dir, dimension, normal| {
                let mut their_chunk =
                    match self.dimension_state.get_chunks_mut().get_mut(&neighbor) {
                        Some(ChunkState::Stasis { neighbors, chunk }) => {
                            *neighbors = (*neighbors + 1).clamp(0, 6);
                            chunk
                        }
                        Some(ChunkState::Active { chunk }) => chunk,
                        _ => return,
                    };

                *neighbors_present += 1;
            });*/

            let blocks = rle::decode(bytes);

            let new_state = ChunkState::Active {
                chunk, /*neighbors: neighbors_present*/
            };

            self.dimension_state
                .get_chunks_mut()
                .insert(position, new_state);

            let blocks = blocks    .into_iter()
                .enumerate()
                .map(|(i, b)| (position, Chunk::delinearize(Chunk::axis(lod), i), b))
                .collect::<Vec<_>>();


            self.dimension_state.set_blocks(&blocks);

            self.dimension_state
                .get_chunk_activations_mut()
                .insert(position);
        }
        for message in client.get(|msg| matches!(msg, Message::Chunk(ChunkMessage::Updated(_)))) {
            let Message::Chunk(ChunkMessage::Updated(ChunkUpdated { position, mut bytes })) = message else {
                continue;
            };

            let lod = match self.dimension_state.get_chunks().get(&position) {
                Some(ChunkState::Active { chunk }) => chunk.lod_level(),
                _ => continue,
            };

            let mut blocks = rle::decode(bytes)
                .into_iter()
                .enumerate()
                .map(|(i, b)| (position, Chunk::delinearize(Chunk::axis(lod), i), b))
                .collect::<Vec<_>>();

            self.dimension_state.set_blocks(&blocks);

            self.dimension_state
                .get_chunk_updated_mut()
                .insert(position);
        }
        for position in self.dimension_state.get_chunk_updated_mut().iter().copied().collect::<Vec<_>>() {
            let Some(mut state) = self.dimension_state.get_chunks_mut().get_mut(&position) else {
                continue;
            };
            let chunk = match &mut state {
                ChunkState::Active { chunk } => chunk,
                ChunkState::Stasis { chunk, .. } => chunk,
                _ => continue,
            };
            for i in 0..Chunk::size(chunk.lod_level()) {
                let visible_mask = calc_block_visible_mask_inside_chunk(&chunk, i);
                let ambient = calc_ambient_inside_chunk(&chunk, i);
                let mut info = chunk.get_mut(i);
                info.visible_mask = visible_mask;
                info.ambient = ambient;
            }
        }
        self.dimension_state.flush_set_blocks();
    }

    pub fn display(&mut self, registry: &mut Registry) {
        let Some(graphics) = registry.resource_mut::<Graphics>() else {
            return;
        };
        for (position, dirs) in self
            .dimension_state
            .get_chunk_border_change_mut()
            .drain()
            .collect::<Vec<_>>()
        {
            neighbors(position, |neighbor, dir, dimension, normal| {
                if !dirs.contains(&dir) {
                    return;
                }

                //do ao calc
            });
        }
        let activated = self
            .dimension_state
            .get_chunk_activations_mut()
            .drain()
            .collect::<Vec<_>>();
        let updated = self
            .dimension_state
            .get_chunk_updated_mut()
            .drain()
            .collect::<Vec<_>>();
        for &position in &activated {
            let entity = registry.spawn();

            let translation = nalgebra::convert::<_, PrecisePosition>(position) * CHUNK_AXIS as f32;

            registry.insert(entity, Translation(translation));
            registry.insert(entity, ChunkMarker);

            self.dimension_state
                .chunk_entity_mapping
                .insert(position, entity);
            self.dimension_state
                .entity_chunk_mapping
                .insert(entity, position);
        }
        for &position in updated.iter().chain(activated.iter()) {
            let entity = self.dimension_state.chunk_entity_mapping[&position];
            let Some(state) = self.dimension_state.get_chunks().get(&position) else {
                continue;
            };
            let chunk = match &state {
                ChunkState::Active { chunk } => chunk,
                ChunkState::Stasis { chunk, .. } => chunk,
                _ => continue,
            };
            let (vertices, indices) =
                gen_block_mesh(chunk, |block, dir| graphics.block_mapping(block, dir));
            if registry.get::<Mesh>(entity).is_some() {
                graphics.destroy_block_mesh(registry.remove::<Mesh>(entity).unwrap());
            }
            registry.insert(
                entity,
                graphics.create_block_mesh(BlockMesh {
                    vertices,
                    indices,
                    position: registry.get::<Translation>(entity).unwrap().0,
                }),
            );
        }
    }
}

pub struct ServerWorld {
    dimension_state: DimensionState,
    pre_generator: (Sender<GenReq>, Receiver<GenReq>),
    post_generator: (Sender<GenResp>, Receiver<GenResp>),
    _generators: Vec<JoinHandle<()>>,
    load_chunk_order: VecDeque<ChunkPosition>,
}

impl ServerWorld {
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
            dimension_state: DimensionState {
                dimension: Dimension::new(),
                chunk_entity_mapping: Default::default(),
                entity_chunk_mapping: Default::default(),
            },
            pre_generator: (pre_generator_tx, pre_generator_rx),
            post_generator: (post_generator_tx, post_generator_rx),
            load_chunk_order: VecDeque::new(),
            _generators,
        }
    }

    pub fn tick(&mut self, registry: &mut Registry, delta_time: f32) {
        let server = registry.resource_mut::<Server>().unwrap();
        if let Err(e) = server.recv() {
            println!("{:?}", e);
            return;
        };

        common_tick::<ServerTag>(registry, delta_time);

        let Ok(new_clients) = server.accept() else {
            panic!("?");
        };

        for &client in &new_clients {
            let entity = registry.spawn();
            registry.insert(entity, Translation(SVector::<f32, 3>::new(0.0, 0.0, 20.0)));
            registry.insert(entity, Look::default());
            registry.insert(entity, Input::default());
            registry.insert(
                entity,
                Loader {
                    load_distance: 4,
                    last_translation_f: SVector::<f32, 3>::new(f32::MAX, f32::MAX, f32::MAX),
                    recalculate_needed_chunks: false,
                    chunk_needed_iter: Box::new(0..0),
                },
            );
            registry.insert(entity, Speed(10.4));
            registry.insert(entity, client);
            registry.insert(entity, ServerTag);

            for (&position, chunk) in
                self.dimension_state
                    .get_chunks()
                    .iter()
                    .filter_map(|(position, state)| {
                        Some((
                            position,
                            match state {
                                ChunkState::Active { chunk } => chunk,
                                _ => None?,
                            },
                        ))
                    })
            {
                let mut blocks = vec![];
                for i in 0..Chunk::size(chunk.lod_level()) {
                    blocks.push(chunk.get(i).block);
                }
                let blocks = rle::encode(&blocks);
                server
                    .send(
                        client,
                        Message::Chunk(ChunkMessage::Activated(ChunkActivated {
                            position,
                            lod: chunk.lod_level(),
                            bytes: blocks,
                        })),
                    )
                    .unwrap();
            }
        }

        for (reg_input, client) in <(&mut Input, &ClientId)>::query(registry) {
            for message in server.get(*client, |msg| matches!(msg, Message::Input(input))) {
                let Message::Input(input) = message else {
                    continue;
                };
                *reg_input = input;
            }
        }

        let mut add_change = HashMap::new();
        for (entity, client) in <(Entity, &ClientId)>::query(registry) {
            for message in server.get(*client, |msg| matches!(msg, Message::Change(_))) {
                let Message::Change(change) = message else {
                    continue;
                };
                add_change.insert(entity,change);
            }
        }
        for (entity, change) in add_change {
            registry.insert(entity, change);
        }
        let mut remove_change = HashSet::new();
        let mut block_changes = Vec::<(ChunkPosition, LocalPosition, Block)>::new();
        for (entity, Translation(translation), Look(look), change) in <(Entity, &Translation, &Look, &Change)>::query(registry) {
                use Change::*;
                let target = match change {
                    Place(_) => raycast::Target::Backstep,
                    Break(_) => raycast::Target::Position,
                };
                let Some((chunk_position, local_position)) = self.dimension_state.raycast(target, *translation, *look) else {
                    continue;
                };

                block_changes.push((
                    chunk_position,
                    local_position,
                    match change {
                        Place(b) => *b,
                        Break(b) => *b,
                    },
                ));
                remove_change.insert(entity);
        }
        for entity in remove_change {
            registry.remove::<Change>(entity);
        }
        self.dimension_state.set_blocks(&block_changes);

        

        let activations = self
            .dimension_state
            .get_chunk_activations_mut()
            .drain()
            .collect::<Vec<_>>();
        let chunk_activated_positions = activations.iter().copied().collect::<HashSet<_>>();
        let activations = activations.into_iter().filter_map(|position| {
            let Some(ChunkState::Active { chunk }) = self.dimension_state.get_chunks().get(&position) else {
                return None;
            };
            Some((position, chunk))
        }).collect::<Vec<_>>();

        if activations.len() > 0 {
            for &client in <(&ClientId)>::query(registry) {
                if new_clients.contains(&client) {
                    continue;
                }
                for (position, chunk) in &activations {
                    let mut blocks = vec![];
                    for i in 0..Chunk::size(chunk.lod_level()) {
                        blocks.push(chunk.get(i).block);
                    }
                    let bytes = rle::encode(&blocks);
                    server
                        .send(
                            client,
                            Message::Chunk(ChunkMessage::Activated(ChunkActivated {
                                position: *position,
                                lod: chunk.lod_level(),
                                bytes,
                            })),
                        )
                        .unwrap();
                }
            }
        }

        let updated = self.dimension_state.get_chunk_updated_mut().drain().collect::<Vec<_>>().into_iter().filter_map(|position| {
            let Some(ChunkState::Active { chunk }) = self.dimension_state.get_chunks().get(&position) else {
                return None;
            };
            if chunk_activated_positions.contains(&position) {
                return None;
            }
            Some((position, chunk))
        }).collect::<Vec<_>>();

        if updated.len() > 0 {
            for &client in <(&ClientId)>::query(registry) {
                if new_clients.contains(&client) {
                    continue;
                }

                for (position, chunk) in &updated {
                    let mut blocks = vec![];
                    for i in 0..Chunk::size(chunk.lod_level()) {
                        blocks.push(chunk.get(i).block);
                    }
                    let bytes = rle::encode(&blocks);
                    server
                        .send(
                            client,
                            Message::Chunk(ChunkMessage::Updated(ChunkUpdated {
                                position: *position,
                                bytes,
                            })),
                        )
                        .unwrap();
                }
            }
        }
        /*
        {

            let mut block_changes = Vec::<(WorldPosition, Block)>::new();
            for (e, Translation(translation), Look(look), change) in
                <(Entity, &Translation, &Look, &Change)>::query(registry)
            {
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
        }
        for chunk in <&mut Chunk>::query(registry) {
            chunk.tick();
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
                    if !self.dimension_state.get_chunks().get(&pos).is_some() {
                        self.load_chunk_order.push_back(pos);
                    }
                    a += 1;
                    if a >= 10 {
                        break;
                    }
                }
            }
        }
        let (pre_generator_tx, _) = &self.pre_generator;
        for position in self.load_chunk_order.drain(..) {
            let lod = 0;

            let _ = pre_generator_tx.send(GenReq { position, lod });

            self.dimension_state
                .get_chunks_mut()
                .insert(position, ChunkState::Generating);
        }
        /*for (entity, chunk, Translation(translation), _) in
            <(Entity, &Chunk, &Translation, Without<Generating>)>::query(registry)
        {
            let lod = (translation_f.metric_distance(&translation) / LOD_VIEW_FACTOR) as usize;
            if chunk.lod().ilog2() as usize != lod {
                let _ = pre_generator_tx.send(GenReq {
                    position: SVector::<isize, 3>::new(
                        translation.x as isize / CHUNK_AXIS as isize,
                        translation.y as isize / CHUNK_AXIS as isize,
                        translation.z as isize / CHUNK_AXIS as isize,
                    ),
                    lod,
                });
                needs_regeneration.insert((entity, lod));
            }
        }
        for (entity, lod) in needs_regeneration {
            registry.remove::<Neighbors>(entity);
            registry.remove::<Chunk>(entity);
            registry.remove::<Active>(entity);
            registry.remove::<Stasis>(entity);
            registry.insert(entity, Chunk::new(lod));
            registry.insert(entity, Neighbors(0));
            registry.insert(entity, Generating);
        }*/
        {
            let (_, post_generator_rx) = &self.post_generator;
            while let Ok(GenResp { position, set, lod }) = post_generator_rx.try_recv() {
                self.dimension_state.get_chunks_mut().insert(
                    position,
                    ChunkState::Stasis {
                        chunk: Chunk::new(lod),
                        neighbors: 0,
                    },
                );
                self.dimension_state
                    .set_blocks(&set.into_iter().collect::<Vec<_>>());

                let Some(mut state) = self.dimension_state.get_chunks_mut().remove(&position) else {
                    continue;
                };

                let ChunkState::Stasis { neighbors: neighbors_present, chunk: my_chunk } = &mut state else {
                    continue;           
                };

                neighbors(position, |neighbor, dir, dimension, normal| {
                    let mut their_chunk =
                        match self.dimension_state.get_chunks_mut().get_mut(&neighbor) {
                            Some(ChunkState::Stasis { neighbors, chunk }) => {
                                *neighbors = (*neighbors + 1).clamp(0, 6);
                                chunk
                            }
                            Some(ChunkState::Active { chunk }) => chunk,
                            _ => return,
                        };

                    *neighbors_present += 1;
                });

                drop(my_chunk);
                drop(neighbors_present);
                self.dimension_state
                    .get_chunks_mut()
                    .insert(position, state);
            }
        }

        let mut activate = HashSet::new();
        for (position, state) in self
            .dimension_state
            .get_chunks()
            .iter()
            .filter(|(_, state)| matches!(state, ChunkState::Stasis { .. }))
        {
            let ChunkState::Stasis { neighbors, .. } = state else {
                continue;
            };
            if *neighbors == 6 {
                activate.insert(*position);
            }
        }
        for position in activate {
            let ChunkState::Stasis { chunk, .. } = self.dimension_state.get_chunks_mut().remove(&position).unwrap() else {
                continue;
            };

            let new_state = ChunkState::Active { chunk };

            self.dimension_state
                .get_chunks_mut()
                .insert(position, new_state);

            self.dimension_state
                .get_chunk_activations_mut()
                .insert(position);
        }

    }
}

pub struct GenReq {
    position: SVector<isize, 3>,
    lod: usize,
}

pub struct GenResp {
    position: SVector<isize, 3>,
    set: HashSet<(ChunkPosition, LocalPosition, Block)>,
    lod: usize,
}

pub fn generator(post_generator_tx: Sender<GenResp>, pre_generator_rx: Receiver<GenReq>) {
    loop {
        let Ok(GenReq { position, lod }) = pre_generator_rx.try_recv() else {
            thread::sleep(Duration::from_millis(1));
            continue;
        };

        let set = gen(position, lod);

        let _ = post_generator_tx.send(GenResp { position, set, lod });
    }
}
