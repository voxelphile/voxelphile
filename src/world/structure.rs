use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    iter,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use band::{Entity, QueryExt, Registry};
use nalgebra::SVector;
use noise::{Fbm, Simplex};
use std::ops;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::{
    graphics::{vertex::BlockVertex, BlockMesh},
    net::Server,
    world::{entity::Electric, LocalPosition},
};

use super::{block::Block, ChunkPosition};

#[repr(u8)]
#[derive(EnumIter, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Forward,
    Back,
    Up,
    Down,
}

impl Direction {
    pub fn opposite(self) -> Self {
        use Direction::*;
        match self {
            Left => Right,
            Right => Left,
            Forward => Back,
            Back => Forward,
            Up => Down,
            Down => Up,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ClientBlockInfo {
    pub block: Block,
    pub visible_mask: u8,
    pub ambient: [u8; 6],
}

impl<'a> BlockRef for &'a ClientBlockInfo {
    fn block_ref(&self) -> &Block {
        &self.block
    }
}

impl<'a> BlockMut for &'a mut ClientBlockInfo {
    fn block_ref(&self) -> &Block {
        &self.block
    }
    fn block_mut(&mut self) -> &mut Block {
        &mut self.block
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ServerBlockInfo {
    pub block: Block,
    pub rotation: u8,
}

impl<'a> BlockRef for &'a ServerBlockInfo {
    fn block_ref(&self) -> &Block {
        &self.block
    }
}

impl<'a> BlockMut for Mut<'a> {
    fn block_ref(&self) -> &Block {
        &self.block
    }
    fn block_mut(&mut self) -> &mut Block {
        &mut self.block
    }
}

fn unpack_ambient(dir: Direction, vertex: usize, ambient: [u8; 6]) -> SVector<f32, 4> {
    SVector::<f32, 4>::new(
        ((ambient[dir as u8 as usize] >> (vertex * 2)) & 3) as f32 / 3.0,
        ((ambient[dir as u8 as usize] >> (vertex * 2)) & 3) as f32 / 3.0,
        ((ambient[dir as u8 as usize] >> (vertex * 2)) & 3) as f32 / 3.0,
        1.0,
    )
}

pub const CHUNK_AXIS: usize = 32;
pub const CHUNK_SIZE: usize = CHUNK_AXIS * CHUNK_AXIS * CHUNK_AXIS;

pub fn chunk_axis(lod: usize) -> SVector<usize, 3> {
    SVector::<usize, 3>::new(CHUNK_AXIS, CHUNK_AXIS, CHUNK_AXIS) / chunk_lod(lod)
}
pub fn chunk_size(lod: usize) -> usize {
    let axis = chunk_axis(lod);
    axis.x * axis.y * axis.z
}
pub fn chunk_lod(lod: usize) -> usize {
    2usize.pow(lod as _) as _
}
pub fn linearize(axis: SVector<usize, 3>, v: SVector<usize, 3>) -> usize {
    (v[2] * axis[1] + v[1]) * axis[0] + v[0]
}
pub fn delinearize(axis: SVector<usize, 3>, i: usize) -> SVector<usize, 3> {
    let mut idx = i;
    let mut v = SVector::<usize, 3>::new(0, 0, 0);
    v[2] = idx / (axis[0] * axis[1]);
    idx -= (v[2] * axis[0] * axis[1]);
    v[1] = idx / axis[0];
    v[0] = idx % axis[0];
    v
}

pub trait BlockRef {
    fn block_ref(&self) -> &Block;
}

pub trait BlockMut {
    fn block_ref(&self) -> &Block;
    fn block_mut(&mut self) -> &mut Block;
}

pub trait Chunk {
    type Ref<'a>: BlockRef
    where
        Self: 'a;
    type Mut<'a>: BlockMut
    where
        Self: 'a;
    fn new(lod: usize) -> Self;
    fn tick(&mut self);
    fn get<'a>(&'a self, i: usize) -> Self::Ref<'a>;
    fn get_mut<'a>(&'a mut self, i: usize) -> Self::Mut<'a>;
    fn lod_level(&self) -> usize {
        0
    }
}

pub struct ClientChunk {
    lod: usize,
    data: Vec<ClientBlockInfo>,
    pub inner_dirty: bool,
    pub neighbor_direction_visibility_mask: u8,
    pub neighbor_direction_ao_mask: u8,
}

impl Chunk for ClientChunk {
    type Ref<'a> = &'a ClientBlockInfo;
    type Mut<'a> = &'a mut ClientBlockInfo;
    fn new(lod: usize) -> Self {
        let lod = lod.min(CHUNK_AXIS.ilog2() as usize);
        let axis = SVector::<usize, 3>::new(CHUNK_AXIS, CHUNK_AXIS, CHUNK_AXIS)
            / 2usize.pow(lod as _) as usize;
        Self {
            lod,
            data: vec![Default::default(); axis.x * axis.y * axis.z],
            neighbor_direction_ao_mask: 0,
            neighbor_direction_visibility_mask: 0,
            inner_dirty: true,
        }
    }
    fn tick(&mut self) {}

    fn get<'a>(&'a self, i: usize) -> Self::Ref<'a> {
        &self.data[i]
    }

    fn get_mut<'a>(&'a mut self, i: usize) -> Self::Mut<'a> {
        &mut self.data[i]
    }

    fn lod_level(&self) -> usize {
        self.lod
    }
}
pub struct ServerChunk {
    data: Vec<ServerBlockInfo>,
    tiles: HashMap<Entity, usize>,
    mapping: HashMap<usize, Entity>,
    registry: Registry,
}

impl Chunk for ServerChunk {
    type Ref<'a> = &'a ServerBlockInfo;
    type Mut<'a> = Mut<'a>;
    fn new(_: usize) -> Self {
        let registry = Registry::default();
        Self {
            data: vec![Default::default(); CHUNK_SIZE],
            tiles: Default::default(),
            mapping: Default::default(),
            registry,
        }
    }
    fn tick(&mut self) {
        for (my_entity, Electric(my_power)) in <(Entity, &mut Electric)>::query(&mut self.registry)
        {
            let my_i = self.tiles[&my_entity];
            let block = **self.get(my_i);
            match block {
                Block::Machine => {
                    if *my_power > 0.0 {
                        println!("I am powered! power: {}", my_power);
                        *my_power -= 10.0;
                    }
                }
                Block::Source => {
                    *my_power = 100.0;
                }
                _ => {}
            }
            inner_neighbors(
                delinearize(chunk_axis(0), self.tiles[&my_entity]),
                chunk_axis(0),
                |neighbor, _| {
                    let Some(&their_entity) = self.mapping.get(&linearize(chunk_axis(0), neighbor)) else {
                    return;
                };
                    let Some(Electric(their_power)) = self.registry.get_mut(their_entity) else {
                    return;
                };
                    let avg = (*my_power + *their_power) / 2.0 - 0.01;
                    *my_power = avg.max(0.0);
                    *their_power = avg.max(0.0);
                },
            );
        }
    }

    fn get<'a>(&'a self, i: usize) -> Self::Ref<'a> {
        &self.data[i]
    }

    fn get_mut<'a>(&'a mut self, i: usize) -> Self::Mut<'a> {
        Mut {
            id: *self.data[i],
            chunk: self,
            i,
        }
    }
}

pub struct Mut<'a> {
    chunk: &'a mut ServerChunk,
    id: Block,
    i: usize,
}

impl ops::Deref for ServerBlockInfo {
    type Target = Block;
    fn deref(&self) -> &Self::Target {
        &self.block
    }
}
impl ops::DerefMut for ServerBlockInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.block
    }
}

impl<'a> ops::Deref for Mut<'a> {
    type Target = ServerBlockInfo;
    fn deref(&self) -> &Self::Target {
        &self.chunk.data[self.i]
    }
}

impl<'a> ops::DerefMut for Mut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.chunk.data[self.i]
    }
}

impl<'a> Drop for Mut<'a> {
    fn drop(&mut self) {
        let curr = *self.chunk.data[self.i];
        if self.id != curr {
            if let Some(&entity) = self.chunk.mapping.get(&self.i) {
                self.chunk.registry.despawn(entity);
                dbg!("destroy", entity);
                self.chunk.tiles.remove(&entity);
                self.chunk.mapping.remove(&self.i);
            }

            use Block::*;
            match curr {
                Machine | Wire | Source => {
                    let entity = self.chunk.registry.spawn();
                    dbg!("create", entity);
                    self.chunk.registry.insert(entity, Electric(0.0));

                    self.chunk.tiles.insert(entity, self.i);
                    self.chunk.mapping.insert(self.i, entity);
                }
                _ => {}
            }
        }
    }
}

pub fn neighbors<F: FnMut(SVector<isize, 3>, Direction, usize, isize)>(
    pos: SVector<isize, 3>,
    mut f: F,
) {
    let mut dir_iter = Direction::iter();
    for d in 0..3 {
        for i in (-1..=1).step_by(2) {
            let dir = dir_iter.next().unwrap();

            let mut norm = SVector::<isize, 3>::new(0, 0, 0);
            norm[d] = i;

            let neighbor = pos + norm;

            (f)(neighbor, dir, d, i);
        }
    }
}

fn inner_neighbors<F: FnMut(SVector<usize, 3>, Direction)>(
    pos: SVector<usize, 3>,
    axis: SVector<usize, 3>,
    mut f: F,
) {
    let pos = nalgebra::convert::<_, SVector<isize, 3>>(pos);
    let axis = nalgebra::convert::<_, SVector<isize, 3>>(axis);
    neighbors(pos, |neighbor, dir, _, _| {
        let outside = neighbor[0] < 0
            || neighbor[1] < 0
            || neighbor[2] < 0
            || neighbor[0] >= axis[0]
            || neighbor[1] >= axis[1]
            || neighbor[2] >= axis[2];

        if outside {
            return;
        }

        let neighbor = nalgebra::try_convert::<_, SVector<usize, 3>>(neighbor).unwrap();

        (f)(neighbor, dir);
    })
}

pub fn calc_block_visible_mask_inside_chunk(s: &ClientChunk, i: usize) -> u8 {
    let mut mask = 0xFF;
    inner_neighbors(
        delinearize(chunk_axis(s.lod), i),
        chunk_axis(s.lod),
        |neighbor, dir| {
            let j = linearize(chunk_axis(s.lod), neighbor);
            if s.get(j).block.is_opaque() {
                mask &= !(1 << dir as u8);
            } else {
                mask |= 1 << dir as u8;
            }
        },
    );
    mask
}

fn vertex_ao(a: usize, b: usize, c: usize) -> usize {
    a + b + usize::max(c, a * b)
}

fn voxel_ao<F: Fn(SVector<isize, 3>) -> bool>(
    position: SVector<isize, 3>,
    d1: SVector<isize, 3>,
    d2: SVector<isize, 3>,
    voxel_present: F,
) -> SVector<u8, 4> {
    let side = SVector::<i32, 4>::new(
        voxel_present(position + d1) as i32,
        voxel_present(position + d2) as i32,
        voxel_present(position - d1) as i32,
        voxel_present(position - d2) as i32,
    );

    let corner = SVector::<i32, 4>::new(
        voxel_present(position + d1 + d2) as i32,
        voxel_present(position - d1 + d2) as i32,
        voxel_present(position - d1 - d2) as i32,
        voxel_present(position + d1 - d2) as i32,
    );

    SVector::<u8, 4>::new(
        vertex_ao(side.x as _, side.y as _, corner.x as _) as _,
        vertex_ao(side.y as _, side.z as _, corner.y as _) as _,
        vertex_ao(side.z as _, side.w as _, corner.z as _) as _,
        vertex_ao(side.w as _, side.x as _, corner.w as _) as _,
    )
}

pub fn calc_ambient_inside_chunk(s: &ClientChunk, i: usize) -> [u8; 6] {
    let mut ambient_values = [0xFF; 6];
    let position = delinearize(chunk_axis(s.lod_level()), i);
    let axis = chunk_axis(s.lod_level());
    inner_neighbors(position, axis, |neighbor, dir| {
        let neighbor = nalgebra::convert::<_, SVector<isize, 3>>(neighbor);
        let position = nalgebra::convert::<_, SVector<isize, 3>>(position);
        let normal = neighbor - position;
        let ambient_data = (voxel_ao)(
            position + normal,
            SVector::<isize, 3>::new(normal.z, normal.x, normal.y),
            SVector::<isize, 3>::new(normal.y, normal.z, normal.x),
            |position| {
                let outside = position[0] < 0
                    || position[1] < 0
                    || position[2] < 0
                    || position[0] >= axis.x as _
                    || position[1] >= axis.y as _
                    || position[2] >= axis.z as _;

                let position = nalgebra::try_convert::<_, SVector<usize, 3>>(position).unwrap();
                !outside && s.get(linearize(axis, position)).block.is_opaque()
            },
        );
        ambient_values[dir as u8 as usize] = (3 - ambient_data.x)
            | (3 - ambient_data.y) << 2
            | (3 - ambient_data.z) << 4
            | (3 - ambient_data.w) << 6;
    });
    ambient_values
}

pub type PolyAmbientValues = Vec<Option<[Option<u8>; 6]>>;

pub fn calc_ambient_between_chunk_neighbors(
    chunk_refs: &[&ClientChunk],
    target: ChunkPosition,
    dir: Direction,
    dimension: usize,
    normal: isize,
) -> PolyAmbientValues {
    profiling::scope!("ambient_between_chunk_neighbors");
    let min = target - ChunkPosition::new(1, 1, 1);

    let target_chunk = chunk_refs[27 / 2];
    let target_axis = chunk_axis(target_chunk.lod_level());
    let mut all_ambient_values =
        PolyAmbientValues::with_capacity(chunk_size(target_chunk.lod_level()));
    all_ambient_values.extend(iter::repeat(None).take(chunk_size(target_chunk.lod_level())));
    let lod = chunk_lod(target_chunk.lod_level());
    'a: for u in 0..CHUNK_AXIS / lod {
        for v in 0..CHUNK_AXIS / lod {
            let mut position = SVector::<usize, 3>::new(0, 0, 0);

            position[dimension] = if normal == 1 {
                chunk_axis(target_chunk.lod_level())[dimension] - 1
            } else {
                0
            };
            position[(dimension + 1) % 3] = u;
            position[(dimension + 2) % 3] = v;

            if target_chunk
                .get(linearize(target_axis, position))
                .visible_mask
                & 63
                == 0
            {
                continue;
            }

            let mut values = [None; 6];
            let mut dir_iter = Direction::iter();
            for d in 0..3 {
                for n in (-1..=1).step_by(2) {
                    let position = nalgebra::convert::<_, SVector<isize, 3>>(position)
                        + target * CHUNK_AXIS as isize;
                    let dir = dir_iter.next().unwrap();
                    let mut normal = SVector::<isize, 3>::new(0, 0, 0);
                    normal[d] = n;

                    let ambient_data = (voxel_ao)(
                        position + normal,
                        SVector::<isize, 3>::new(normal.z, normal.x, normal.y),
                        SVector::<isize, 3>::new(normal.y, normal.z, normal.x),
                        |mut position| {
                            let chunk_position = SVector::<isize, 3>::new(
                                position.x.div_euclid(CHUNK_AXIS as _) as isize,
                                position.y.div_euclid(CHUNK_AXIS as _) as isize,
                                position.z.div_euclid(CHUNK_AXIS as _) as isize,
                            );
                            let diff = chunk_position - min;
                            let i = (diff.z as usize * 3 + diff.y as usize) * 3 + diff.x as usize;
                            let chunk = chunk_refs[i];
                            let axis = chunk_axis(chunk.lod_level());
                            position /= chunk_lod(chunk.lod_level()) as isize;
                            let local_position = SVector::<usize, 3>::new(
                                position.x.rem_euclid(axis.x as _) as usize,
                                position.y.rem_euclid(axis.y as _) as usize,
                                position.z.rem_euclid(axis.z as _) as usize,
                            );
                            chunk
                                .get(linearize(axis, local_position))
                                .block_ref()
                                .is_opaque()
                        },
                    );

                    let value = (3 - ambient_data.x)
                        | (3 - ambient_data.y) << 2
                        | (3 - ambient_data.z) << 4
                        | (3 - ambient_data.w) << 6;
                    values[dir as u8 as usize] = Some(value);
                }
            }
            all_ambient_values[linearize(chunk_axis(target_chunk.lod_level()), position)] =
                Some(values);
        }
    }
    all_ambient_values
}

pub fn set_ambient_between_chunk_neighbors(
    all_ambient_values: PolyAmbientValues,
    target_chunk: &mut ClientChunk,
) {
    for (i, values) in all_ambient_values
        .into_iter()
        .enumerate()
        .filter(|(_, v)| v.is_some())
    {
        for (j, value) in values
            .unwrap()
            .into_iter()
            .enumerate()
            .filter(|(_, v)| v.is_some())
        {
            target_chunk.get_mut(i).ambient[j] = value.unwrap();
        }
    }
}

pub fn are_all_border_blocks_invisible(
    chunk: &ClientChunk,
    dir: Direction,
    dimension: usize,
    normal: isize,
) -> bool {
    let lod = chunk_lod(chunk.lod);
    'a: for u in 0..CHUNK_AXIS / lod {
        for v in 0..CHUNK_AXIS / lod {
            let mut my_block_position = SVector::<usize, 3>::new(0, 0, 0);

            my_block_position[dimension] = if normal == 1 {
                chunk_axis(chunk.lod)[dimension] - 1
            } else {
                0
            };
            my_block_position[(dimension + 1) % 3] = u;
            my_block_position[(dimension + 2) % 3] = v;

            if ((chunk
                .get(linearize(chunk_axis(chunk.lod), my_block_position))
                .visible_mask
                >> (dir as u8))
                & 1)
                == 1
            {
                return false;
            }
        }
    }
    true
}

pub fn calc_block_visible_mask_between_chunks(
    chunk: &mut ClientChunk,
    neighbor: &mut ClientChunk,
    dir: Direction,
    dimension: usize,
    normal: isize,
) -> bool {
    let mut changed = false;
    let max_lod = chunk_lod(chunk.lod).max(chunk_lod(neighbor.lod));
    let min_lod = chunk_lod(chunk.lod).min(chunk_lod(neighbor.lod));
    let (small_chunk, large_chunk, same) = if chunk_lod(chunk.lod) == min_lod {
        (neighbor, chunk, false)
    } else {
        (chunk, neighbor, true)
    };

    let (normal_eq, my_dir) = if same { (1, dir) } else { (-1, dir.opposite()) };
    let their_dir = my_dir.opposite();

    let ratio_lod = max_lod / min_lod;
    for u in 0..CHUNK_AXIS / max_lod {
        for v in 0..CHUNK_AXIS / max_lod {
            let mut my_block_position = SVector::<usize, 3>::new(0, 0, 0);
            let mut their_block_position = SVector::<usize, 3>::new(0, 0, 0);

            my_block_position[dimension] = if normal == normal_eq {
                chunk_axis(small_chunk.lod)[dimension] - 1
            } else {
                0
            };
            my_block_position[(dimension + 1) % 3] = u;
            my_block_position[(dimension + 2) % 3] = v;

            their_block_position[dimension] = if normal == normal_eq {
                0
            } else {
                chunk_axis(large_chunk.lod)[dimension] - 1
            };
            their_block_position[(dimension + 1) % 3] = u * ratio_lod;
            their_block_position[(dimension + 2) % 3] = v * ratio_lod;

            let mut my_block_ref =
                small_chunk.get_mut(linearize(chunk_axis(small_chunk.lod), my_block_position));

            let ClientBlockInfo {
                block: my_block,
                visible_mask: my_visible_mask,
                ..
            } = &mut *my_block_ref;
            let my_visible_mask_reference = *my_visible_mask;
            *my_visible_mask &= !(1 << my_dir as u8);
            for u2 in 0..ratio_lod {
                for v2 in 0..ratio_lod {
                    let mut their_offset = SVector::<usize, 3>::new(0, 0, 0);

                    their_offset[(dimension + 1) % 3] = u2;
                    their_offset[(dimension + 2) % 3] = v2;

                    let their_real_position = their_block_position + their_offset;

                    let mut their_block_ref = large_chunk
                        .get_mut(linearize(chunk_axis(large_chunk.lod), their_real_position));
                    let ClientBlockInfo {
                        block: their_block,
                        visible_mask: their_visible_mask,
                        ..
                    } = &mut *their_block_ref;

                    let their_visible_mask_reference = *their_visible_mask;

                    if my_block.is_opaque() {
                        *their_visible_mask &= !(1 << their_dir as u8);
                    } else {
                        *their_visible_mask |= 1 << their_dir as u8;
                    }

                    if !their_block.is_opaque() {
                        *my_visible_mask |= 1 << my_dir as u8;
                    }

                    changed = changed
                        || *my_visible_mask != my_visible_mask_reference
                        || *their_visible_mask != their_visible_mask_reference;
                }
            }
        }
    }
    changed
}

pub fn gen(position: SVector<isize, 3>, lod: usize) -> ServerChunk {
    let mut chunk = ServerChunk::new(lod);
    let axis = chunk_axis(lod);

    let mut alpha = Fbm::<Simplex>::new(400);
    let mut beta = Fbm::<Simplex>::new(500);

    alpha.frequency = 0.0005;
    beta.frequency = 0.0005;

    let translation = position * CHUNK_AXIS as isize;

    let noise_scale = SVector::<isize, 3>::new(8, 8, 8);
    let noise_axis = SVector::<isize, 3>::new(
        (CHUNK_AXIS as isize) / noise_scale[0] + 1,
        (CHUNK_AXIS as isize) / noise_scale[1] + 1,
        (CHUNK_AXIS as isize) / noise_scale[2] + 1,
    );

    let mut noise_values = vec![0f64; (noise_axis[0] * noise_axis[1] * noise_axis[2]) as usize];

    for x in 0..noise_axis[0] {
        for y in 0..noise_axis[1] {
            for z in 0..noise_axis[2] {
                let local_position = SVector::<isize, 3>::new(
                    (x - 1) * noise_scale[0],
                    (y - 1) * noise_scale[1],
                    (z - 1) * noise_scale[2],
                );
                noise_values[(x + noise_axis[0] * (y + noise_axis[1] * z)) as usize] =
                    world_gen_base(translation + local_position, &alpha, &beta);
            }
        }
    }

    for i in 0..chunk_size(lod) {
        use lerp::Lerp;
        let local_position = delinearize(chunk_axis(lod), i);
        let adjusted_position =
            nalgebra::convert::<_, SVector<isize, 3>>(local_position) * chunk_lod(lod) as isize;

        let noise_interp = SVector::<f64, 3>::new(
            (adjusted_position[0] as f64 / noise_scale[0] as f64).fract(),
            (adjusted_position[1] as f64 / noise_scale[1] as f64).fract(),
            (adjusted_position[2] as f64 / noise_scale[2] as f64).fract(),
        );

        let noise_position0 = SVector::<isize, 3>::new(
            adjusted_position[0] / noise_scale[0],
            adjusted_position[1] / noise_scale[1],
            adjusted_position[2] / noise_scale[2],
        );

        let noise_position1 = SVector::<isize, 3>::new(
            (noise_position0[0] + 1).min(noise_axis[0] as isize),
            (noise_position0[1] + 1).min(noise_axis[1] as isize),
            (noise_position0[2] + 1).min(noise_axis[2] as isize),
        );

        let d000 = noise_values[(noise_position0[0]
            + noise_axis[0] * (noise_position0[1] + noise_axis[1] * noise_position0[2]))
            as usize];
        let d100 = noise_values[(noise_position1[0]
            + noise_axis[0] * (noise_position0[1] + noise_axis[1] * noise_position0[2]))
            as usize];

        let d010 = noise_values[(noise_position0[0]
            + noise_axis[0] * (noise_position1[1] + noise_axis[1] * noise_position0[2]))
            as usize];
        let d110 = noise_values[(noise_position1[0]
            + noise_axis[0] * (noise_position1[1] + noise_axis[1] * noise_position0[2]))
            as usize];

        let d001 = noise_values[(noise_position0[0]
            + noise_axis[0] * (noise_position0[1] + noise_axis[1] * noise_position1[2]))
            as usize];
        let d101 = noise_values[(noise_position1[0]
            + noise_axis[0] * (noise_position0[1] + noise_axis[1] * noise_position1[2]))
            as usize];

        let d011 = noise_values[(noise_position0[0]
            + noise_axis[0] * (noise_position1[1] + noise_axis[1] * noise_position1[2]))
            as usize];
        let d111 = noise_values[(noise_position1[0]
            + noise_axis[0] * (noise_position1[1] + noise_axis[1] * noise_position1[2]))
            as usize];

        let density = f64::lerp(
            f64::lerp(
                f64::lerp(d000, d100, noise_interp[0]),
                f64::lerp(d010, d110, noise_interp[0]),
                noise_interp[1],
            ),
            f64::lerp(
                f64::lerp(d001, d101, noise_interp[0]),
                f64::lerp(d011, d111, noise_interp[0]),
                noise_interp[1],
            ),
            noise_interp[2],
        );

        let i = linearize(axis, local_position);
        if density > 0.0 {
            chunk.get_mut(i).block = Block::Stone;
        } else {
            chunk.get_mut(i).block = Block::Air;
        }
    }
    chunk
}

pub fn cubic_block<F: Fn(Block, Direction) -> Option<u32> + Copy>(
    info: &ClientBlockInfo,
    lod: usize,
    position: SVector<usize, 3>,
    block_mapping: F,
    block_vertices: &mut Vec<BlockVertex>,
    block_indices: &mut Vec<u32>,
) {
    const VERTEX_OFFSETS: [SVector<usize, 3>; 8] = [
        SVector::<usize, 3>::new(0, 0, 1), //0
        SVector::<usize, 3>::new(0, 1, 1), //1
        SVector::<usize, 3>::new(1, 1, 1), //2
        SVector::<usize, 3>::new(1, 0, 1), //3
        SVector::<usize, 3>::new(0, 0, 0), //4
        SVector::<usize, 3>::new(0, 1, 0), //5
        SVector::<usize, 3>::new(1, 1, 0), //6
        SVector::<usize, 3>::new(1, 0, 0), //7
    ];

    const VERTEX_SIDE_ORDER: [[usize; 4]; 6] = [
        [4, 5, 1, 0],
        [3, 2, 6, 7],
        [0, 3, 7, 4],
        [5, 6, 2, 1],
        [5, 4, 7, 6],
        [0, 1, 2, 3],
    ];

    let norm_eq = |dir: Direction, i: usize| -> usize {
        use Direction::*;
        (match dir {
            Back => [1, 2, 3, 0],
            Forward => [2, 1, 3, 0],
            Left => [1, 0, 3, 2],
            Right => [0, 1, 2, 3],
            Up => [2, 1, 0, 3],
            //todo
            Down => [0, 3, 1, 2],
            _ => panic!("yo"),
        })[i]
    };

    for (i, dir) in Direction::iter().enumerate() {
        if ((info.visible_mask >> dir as u8) & 1) == 1 {
            let block_vertex_count = block_vertices.len() as u32;
            block_vertices.extend([
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][0]] * lod + position * lod,
                    lod,
                    SVector::<f32, 2>::new(0.0, 0.0),
                    dir as u8,
                    (block_mapping)(info.block, dir).unwrap_or_default(),
                    unpack_ambient(dir, (norm_eq)(dir.opposite(), 0), info.ambient),
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][1]] * lod + position * lod,
                    lod,
                    SVector::<f32, 2>::new(0.0, 1.0),
                    dir as u8,
                    (block_mapping)(info.block, dir).unwrap_or_default(),
                    unpack_ambient(dir, (norm_eq)(dir.opposite(), 1), info.ambient),
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][2]] * lod + position * lod,
                    lod,
                    SVector::<f32, 2>::new(1.0, 1.0),
                    dir as u8,
                    (block_mapping)(info.block, dir).unwrap_or_default(),
                    unpack_ambient(dir, (norm_eq)(dir.opposite(), 2), info.ambient),
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][3]] * lod + position * lod,
                    lod,
                    SVector::<f32, 2>::new(1.0, 0.0),
                    dir as u8,
                    (block_mapping)(info.block, dir).unwrap_or_default(),
                    unpack_ambient(dir, (norm_eq)(dir.opposite(), 3), info.ambient),
                ),
            ]);

            block_indices.extend([
                block_vertex_count + 1,
                block_vertex_count + 0,
                block_vertex_count + 3,
                block_vertex_count + 1,
                block_vertex_count + 3,
                block_vertex_count + 2,
            ]);
        }
    }
}

pub fn gen_block_mesh<F: Fn(Block, Direction) -> Option<u32> + Copy>(
    s: &ClientChunk,
    block_mapping: F,
) -> (Vec<BlockVertex>, Vec<u32>) {
    let mut block_vertices = vec![];
    let mut block_indices = vec![];

    for i in 0..chunk_size(s.lod_level()) {
        let info = s.get(i);
        let p = delinearize(chunk_axis(s.lod), i);
        if (*info).block.is_opaque() {
            cubic_block(
                info,
                chunk_lod(s.lod),
                p,
                block_mapping,
                &mut block_vertices,
                &mut block_indices,
            );
        }
    }
    (block_vertices, block_indices)
}

fn map(value: f32, min1: f32, max1: f32, min2: f32, max2: f32) -> f32 {
    return min2 + (value - min1) * (max2 - min2) / (max1 - min1);
}

fn world_gen_base(position: SVector<isize, 3>, alpha: &Fbm<Simplex>, beta: &Fbm<Simplex>) -> f64 {
    use noise::NoiseFn;

    let y = beta.get([position[0] as f64, position[1] as f64, position[2] as f64]) as f32;

    let a = alpha.get([position[0] as f64, position[1] as f64, position[2] as f64]);

    let squish_factor = map(y, -0.9, 0.9, 0.0008, 0.009) as f64;

    let height_offset = 0.0;

    let density = a;
    let density_mod = squish_factor * (height_offset - position[2] as f64);

    density + density_mod
}
