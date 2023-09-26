pub mod block;
pub mod dimension;
pub mod entity;
pub mod raycast;
pub mod structure;

use self::block::*;
use self::dimension::{ChunkState, Dimension};
use crate::graphics::{BlockMesh, Graphics, GraphicsInterface, Mesh};
use crate::input::{Input, Inputs};
use crate::net::{
    ChunkActivated, ChunkMessage, ChunkUpdated, Client, ClientId, ClientTag, Correct, Message,
    Server, ServerTag,
};
use crate::util::rle;
use crate::world::entity::{Break, Change};
use crate::world::structure::*;
use crate::{world, FIXED_TIME};
use band::{QueryExt, *};
use crossbeam::channel::{self, Receiver, Sender};
use lerp::num_traits::ToBytes;
use nalgebra::{Dim, SVector, Unit, UnitQuaternion};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Instant, SystemTime};
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
    time::Duration,
};
use std::{fmt, iter, thread};
use std::{mem, ops};
use structure::Chunk;
use strum::IntoEnumIterator;

use self::entity::{
    ChunkTranslation, Dirty, Display, Loader, Look, Main, Observer, Speed, Target, Translation,
};
use self::raycast::Ray;
use self::structure::{
    calc_block_visible_mask_between_chunks, calc_block_visible_mask_inside_chunk, gen,
    gen_block_mesh, neighbors, ClientBlockInfo, ServerBlockInfo, CHUNK_AXIS,
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

pub struct DimensionState<C: Chunk> {
    chunk_entity_mapping: HashMap<ChunkPosition, Entity>,
    entity_chunk_mapping: HashMap<Entity, ChunkPosition>,
    dimension: Dimension<C>,
}

impl<C: Chunk> ops::Deref for DimensionState<C> {
    type Target = Dimension<C>;
    fn deref(&self) -> &Self::Target {
        &self.dimension
    }
}

impl<C: Chunk> ops::DerefMut for DimensionState<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dimension
    }
}

pub struct ClientWorld {
    dimension_state: DimensionState<ClientChunk>,
}

fn common_fixed_tick<Tag: Component + fmt::Debug>(registry: &mut Registry) {}

#[profiling::all_functions]
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

    pub fn delta_tick(&mut self, registry: &mut Registry, delta_time: f32) {
        for (Target(target), Translation(translation)) in
            <(&Target, &mut Translation)>::query(registry)
        {
            use lerp::Lerp;
            *translation = PrecisePosition::new(
                translation.x.lerp(target.x, 0.5),
                translation.y.lerp(target.y, 0.5),
                translation.z.lerp(target.z, 0.5),
            )
        }
    }

    pub fn fixed_tick(&mut self, registry: &mut Registry, delta_time: f32) {
        let client = registry.resource_mut::<Client>().unwrap();
        let _ = client.recv();

        common_fixed_tick::<ClientTag>(registry);

        if let Some((inputs, _)) = <(&Inputs, &Observer)>::query(registry).next() {
            client.send(Message::Inputs(inputs.clone())).unwrap();
        }
        for (Target(translation), Look(look), Speed(speed), inputs, _) in
            <(&mut Target, &mut Look, &Speed, &mut Inputs, &ClientTag)>::query(registry)
        {
            let Some((mut prev_time, _)) = inputs.state.get(0).copied() else {
                continue;
            };
            let mut input;
            loop {
                input = inputs.state.get(0).map(|(_, b)| *b).unwrap();

                let curr_time = if let Some((time, _)) = inputs.state.get(1) {
                    *time
                } else {
                    SystemTime::now()
                };

                if inputs.state.len() > 1 {
                    inputs.state.pop_front();
                }

                let delta_time = curr_time.duration_since(prev_time).unwrap().as_secs_f32();
                prev_time = curr_time;

                *look += input.gaze;

                let direction = if input.direction.magnitude() == 0.0 {
                    Default::default()
                } else {
                    input.direction.normalize()
                };

                *translation += speed
                    * delta_time
                    * (UnitQuaternion::from_axis_angle(
                        &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
                        look.x,
                    )
                    .to_rotation_matrix()
                        * direction);

                if inputs.state.len() == 1 {
                    break;
                }
            }
            inputs.state[0].1.gaze = Default::default();
            inputs.state[0].0 = SystemTime::now();
        }

        let mut remove_change = None;
        if let Some((entity, change, _)) = <(Entity, &Change, &Observer)>::query(registry).next() {
            client.send(Message::Change(*change)).unwrap();
            remove_change = Some(entity);
        }
        if let Some(entity) = remove_change {
            registry.remove::<Change>(entity);
        }
        for message in client.get(|(msg, _)| matches!(msg, Message::Correct(Correct::Target(_)))) {
            let Message::Correct(Correct::Target(target)) = message.0 else {
                continue;
            };
            let Some((rep_target, _)) = <(&mut Target, &Main)>::query(registry).next() else {
                continue;
            };
            *rep_target = target;
        }
        for message in
            client.get(|(msg, _)| matches!(msg, Message::Chunk(ChunkMessage::Activated(_))))
        {
            let Message::Chunk(ChunkMessage::Activated(ChunkActivated { position, lod, bytes })) = message.0 else {
                continue;
            };

            let mut chunk = ClientChunk::new(lod);

            let mut blocks = rle::decode(bytes);

            for i in 0..chunk_size(chunk.lod_level()) {
                chunk.get_mut(i).block = blocks[i];
            }

            let new_state = ChunkState::Stasis {
                chunk,
            };

            self.dimension_state
                .get_chunks_mut()
                .insert(position, new_state);
        }

        {
            for (position, mut state) in self
                .dimension_state
                .get_chunks_mut()
                .drain_filter(|p, state|  {
                    if !matches!(state, ChunkState::Stasis { .. }) {
                        return false;
                    }

                    match &state {
                        ChunkState::Stasis { chunk, .. } => {
                            chunk.neighbor_direction_visibility_mask != 63
                        }
                        _ => false,
                    }
                })
                .collect::<Vec<_>>()
            {
                let ChunkState::Stasis { chunk } = &mut state else {
            continue;
        };
                neighbors(position, |neighbor, dir, dimension, normal| {
                    if ((chunk.neighbor_direction_visibility_mask >> dir as u8) & 1) == 1 {
                        return;
                    }
                    let Some(neighbor_state) = self
                    .dimension_state
                    .get_chunks_mut()
                    .get_mut(&neighbor) else {
                        return;
                    };

                    let neighbor = match neighbor_state
                    {
                        ChunkState::Active { chunk } => chunk,
                        ChunkState::Stasis { chunk, .. } => chunk,
                        _ => return,
                    };
                    calc_block_visible_mask_between_chunks(chunk, neighbor, dir, dimension, normal);

                    chunk.neighbor_direction_visibility_mask |= 1 << dir as u8;
                });

                drop(chunk);
                self.dimension_state.get_chunks_mut().insert(position, state);
            }

            let mut activate = HashSet::new();

            'a: for position in self
                .dimension_state
                .get_chunks()
                .iter()
                .filter_map(|(p, state)| {
                    if !matches!(state, ChunkState::Stasis { .. }) {
                        return None;
                    }

                    let all_neighbors_processed = match state {
                        ChunkState::Stasis { chunk } => {
                            chunk.neighbor_direction_ao_mask != 63
                        }
                        _ => false,
                    };

                    all_neighbors_processed.then(|| *p)
                })
                .collect::<Vec<_>>()
            {
                let min = position - ChunkPosition::new(1, 1, 1);
                let mut chunk_refs = Vec::<&ClientChunk>::with_capacity(27);
                for i in 0..27 {
                    let pos = min
                        + nalgebra::convert::<_, ChunkPosition>(delinearize(
                            LocalPosition::new(3, 3, 3),
                            i,
                        ));
                    chunk_refs.push(match &self.dimension_state.get_chunks().get(&pos) {
                        Some(ChunkState::Active { chunk }) => chunk,
                            Some(ChunkState::Stasis { chunk, .. }) => chunk,
                        _ => continue 'a,
                    });
                }
                let mut poly_ambient_values = vec![];
                for _ in 0..6 {
                    poly_ambient_values.push(None);
                }
                for dir in Direction::iter() {
                    if ((chunk_refs[27 / 2].neighbor_direction_ao_mask >> dir as u8) & 1) == 1 {
                        continue;
                    }
                    poly_ambient_values[1 << (dir as u8)] =
                        Some(calc_ambient_between_chunk_neighbors(
                            &chunk_refs,
                            chunk_refs[27 / 2].neighbor_direction_ao_mask,
                            position,
                        ));
                }
                
                drop(chunk_refs);

                let Some(ChunkState::Stasis { chunk }) = self.dimension_state.get_chunks_mut().get_mut(&position) else {
            panic!("?");
        };
                chunk.neighbor_direction_ao_mask = 63;
                for ambient_values in poly_ambient_values {
                    let Some(ambient_values) = ambient_values else {
                continue;
            };
                    set_ambient_between_chunk_neighbors(ambient_values, chunk);
                }
                activate.insert(position);
            }

            for position in activate {
                let Some(ChunkState::Stasis { chunk }) = self.dimension_state.get_chunks_mut().get_mut(&position) else {
            panic!("?");
        };
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

        for message in
            client.get(|(msg, _)| matches!(msg, Message::Chunk(ChunkMessage::Updated(_))))
        {
            let Message::Chunk(ChunkMessage::Updated(ChunkUpdated { position, bytes })) = message.0 else {
                continue;
            };

            let chunk = match self.dimension_state.get_chunks_mut().get_mut(&position) {
                Some(ChunkState::Active { chunk }) => chunk,
                _ => continue,
            };

            let mut blocks = rle::decode(bytes);

            for i in 0..chunk_size(chunk.lod_level()) {
                chunk.get_mut(i).block = blocks[i];
            }

            self.dimension_state
                .get_chunk_updated_mut()
                .insert(position);
        }
        for position in self
            .dimension_state
            .get_chunk_updated()
            .iter()
            .chain(self.dimension_state.get_chunk_activations().iter())
            .copied()
            .collect::<Vec<_>>()
        {
            let Some(mut state) = self.dimension_state.get_chunks_mut().get_mut(&position) else {
                continue;
            };
            let chunk = match &mut state {
                ChunkState::Active { chunk } => chunk,
                ChunkState::Stasis { chunk, .. } => chunk,
                _ => continue,
            };
            for i in 0..chunk_size(chunk.lod_level()) {
                let visible_mask = calc_block_visible_mask_inside_chunk(chunk, i);
                let ambient = calc_ambient_inside_chunk(chunk, i);
                let mut info = chunk.get_mut(i);
                info.visible_mask = visible_mask;
                info.ambient = ambient;
            }
        }
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
    dimension_state: DimensionState<ServerChunk>,
    pre_generator: (Sender<GenReq>, Receiver<GenReq>),
    post_generator: (Sender<GenResp>, Receiver<GenResp>),
    _generators: Vec<JoinHandle<()>>,
    load_chunk_order: VecDeque<ChunkPosition>,
}
#[profiling::all_functions]
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

    pub fn delta_tick(&mut self, registry: &mut Registry, delta_time: f32) {}

    pub fn fixed_tick(&mut self, registry: &mut Registry, delta_time: f32) {
        let server = registry.resource_mut::<Server>().unwrap();
        if let Err(e) = server.recv() {
            println!("{:?}", e);
            return;
        };

        common_fixed_tick::<ServerTag>(registry);

        let Ok(new_clients) = server.accept() else {
            panic!("?");
        };

        for (Translation(translation), Look(look), Speed(speed), inputs, _) in
            <(&mut Translation, &mut Look, &Speed, &mut Inputs, &ServerTag)>::query(registry)
        {
            let Some((mut prev_time, _)) = inputs.state.get(0).copied() else {
                continue;
            };
            let mut input;
            loop {
                input = inputs.state.get(0).map(|(_, b)| *b).unwrap();

                let curr_time = if let Some((time, _)) = inputs.state.get(1) {
                    *time
                } else {
                    SystemTime::now()
                };

                if inputs.state.len() > 1 {
                    inputs.state.pop_front();
                }

                let delta_time = curr_time.duration_since(prev_time).unwrap().as_secs_f32();
                prev_time = curr_time;

                *look += input.gaze;

                let direction = if input.direction.magnitude() == 0.0 {
                    Default::default()
                } else {
                    input.direction.normalize()
                };

                *translation += speed
                    * delta_time
                    * (UnitQuaternion::from_axis_angle(
                        &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
                        look.x,
                    )
                    .to_rotation_matrix()
                        * direction);

                if inputs.state.len() == 1 {
                    break;
                }
            }
            inputs.state[0].1.gaze = Default::default();
            inputs.state[0].0 = SystemTime::now();
        }

        for &client in &new_clients {
            let entity = registry.spawn();
            registry.insert(entity, Translation(SVector::<f32, 3>::new(0.0, 0.0, 20.0)));
            registry.insert(entity, Look::default());
            registry.insert(entity, Inputs::default());
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
                for i in 0..chunk_size(0) {
                    blocks.push(chunk.get(i).block);
                }
                let blocks = rle::encode(&blocks);
                server
                    .send(
                        client,
                        Message::Chunk(ChunkMessage::Activated(ChunkActivated {
                            position,
                            lod: 0,
                            bytes: blocks,
                        })),
                    )
                    .unwrap();
            }
        }
        for (rep_inputs, client) in <(&mut Inputs, &ClientId)>::query(registry) {
            for (message, info) in
                server.get(*client, |(msg, _)| matches!(msg, Message::Inputs(input)))
            {
                let Message::Inputs(inputs) = message else {
                    continue;
                };
                *rep_inputs = inputs;
            }
        }
        for (translation, client) in <(&Translation, &ClientId)>::query(registry) {
            server.send(
                *client,
                Message::Correct(Correct::Target(Target(translation.0))),
            );
        }
        let mut add_change = HashMap::new();
        for (entity, client) in <(Entity, &ClientId)>::query(registry) {
            for message in server.get(*client, |(msg, _)| matches!(msg, Message::Change(_))) {
                let Message::Change(change) = message.0 else {
                    continue;
                };
                add_change.insert(entity, change);
            }
        }
        for (entity, change) in add_change {
            registry.insert(entity, change);
        }
        let mut remove_change = HashSet::new();
        let mut block_changes = Vec::<(ChunkPosition, LocalPosition, Block)>::new();
        for (entity, Translation(translation), Look(look), change) in
            <(Entity, &Translation, &Look, &Change)>::query(registry)
        {
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
                    for i in 0..chunk_size(0) {
                        blocks.push(chunk.get(i).block);
                    }
                    let bytes = rle::encode(&blocks);
                    server
                        .send(
                            client,
                            Message::Chunk(ChunkMessage::Activated(ChunkActivated {
                                position: *position,
                                lod: 0,
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
                    for i in 0..chunk_size(0) {
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
        {
            profiling::scope!("calc_load_order");

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
        }
        {
            profiling::scope!("start_gen");
            let (pre_generator_tx, _) = &self.pre_generator;
            for position in self.load_chunk_order.drain(..) {
                let lod = 0;

                let _ = pre_generator_tx.send(GenReq { position, lod });

                self.dimension_state
                    .get_chunks_mut()
                    .insert(position, ChunkState::Generating);
            }
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
            profiling::scope!("recv_gen");
            let (_, post_generator_rx) = &self.post_generator;
            let mut iter = iter::repeat_with(|| post_generator_rx.try_recv()).take(2);
            while let Some(GenResp {
                position,
                chunk,
                lod,
            }) = iter.next().map(Result::ok).flatten()
            {
                self.dimension_state.get_chunks_mut().insert(
                    position,
                    ChunkState::Stasis {
                        chunk,
                    },
                );
                self.dimension_state
                    .get_chunk_updated_mut()
                    .insert(position);
            }
        }

        {
            profiling::scope!("activate");
            let mut activate = HashSet::new();
            for (position, _) in self
                .dimension_state
                .get_chunks()
                .iter()
                .filter(|(_, state)| matches!(state, ChunkState::Stasis { .. }))
            {
                activate.insert(*position);
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
}

pub struct GenReq {
    position: SVector<isize, 3>,
    lod: usize,
}

pub struct GenResp {
    position: SVector<isize, 3>,
    chunk: ServerChunk,
    lod: usize,
}

pub fn generator(post_generator_tx: Sender<GenResp>, pre_generator_rx: Receiver<GenReq>) {
    profiling::register_thread!("generator");
    loop {
        let Ok(GenReq { position, lod }) = pre_generator_rx.try_recv() else {
            thread::sleep(Duration::from_millis(1));
            continue;
        };

        profiling::scope!("gen");

        let chunk = gen(position, lod);

        let _ = post_generator_tx.send(GenResp {
            position,
            chunk,
            lod,
        });
    }
}
