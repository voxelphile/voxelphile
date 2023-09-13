use std::{cell::RefCell, collections::HashMap};

use nalgebra::SVector;
use noise::{Fbm, Simplex};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::graphics::{vertex::BlockVertex, BlockMesh};

#[derive(EnumIter, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Block {
    Air,
    Stone,
    Wood,
}

impl Block {
    pub fn texture_name(&self) -> Option<String> {
        use Block::*;
        Some(String::from(
            match self {
                Stone => "stone",
                //Wood => "wood",
                _ => None?
            }   
        ))
    }
    pub fn parallax(&self) -> bool {
        use Block::*;
        match self {
            Stone => true,
            Wood => true,
            _ => false,
        }
    }
    pub fn normal(&self) -> bool {
        use Block::*;
        match self {
            Stone => true,
            Wood => true,
            _ => false,
        }
    }
    pub fn is_opaque(&self) -> bool {
        !matches!(self, Block::Air)
    }
}

#[repr(u8)]
#[derive(EnumIter, Clone, Copy)]
pub enum NeighborDirection {
    Left,
    Right,
    Forward,
    Back,
    Up,
    Down,
}

impl NeighborDirection {
    pub fn opposite(self) -> Self {
        use NeighborDirection::*;
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

#[derive(Clone, Copy, Debug)]
pub struct BlockInfo {
    pub block: Block,
    pub visible_mask: u8,
}

pub const CHUNK_AXIS: usize = 32;
pub const CHUNK_SIZE: usize = CHUNK_AXIS * CHUNK_AXIS * CHUNK_AXIS;

pub trait Structure {
    fn axis(&self) -> SVector<usize, 3>;
    fn size(&self) -> usize;
    fn contains(&self, _: usize) -> bool {
        true
    }
    fn get(&self, i: usize) -> &BlockInfo;
    fn get_mut(&mut self, i: usize) -> &mut BlockInfo;
    fn linearize(&self, v: SVector<usize, 3>) -> usize {
        let axis = self.axis();
        (v[2] * axis[1] + v[1]) * axis[0] + v[0]
    }
    fn delinearize(&self, i: usize) -> SVector<usize, 3> {
        let mut idx = i;
        let axis = self.axis();
        let mut v = SVector::<usize, 3>::new(0, 0, 0);
        v[2] = idx / (axis[0] * axis[1]);
        idx -= (v[2] * axis[0] * axis[1]);
        v[1] = idx / axis[0];
        v[0] = idx % axis[0];
        v
    }
    fn for_each<F: FnMut(usize, &BlockInfo)>(&self, mut f: F) {
        for i in 0..self.size() {
            (f)(i, self.get(i));
        }
    }
    fn for_each_mut<F: FnMut(usize, &mut BlockInfo)>(&mut self, mut f: F) {
        for i in 0..self.size() {
            (f)(i, self.get_mut(i));
        }
    }
}

pub struct Blueprint {
    data: HashMap<SVector<isize, 3>, BlockInfo>,
    minimum: SVector<isize, 3>,
    maximum: SVector<isize, 3>,
}

impl Structure for Blueprint {
    fn axis(&self) -> SVector<usize, 3> {
        nalgebra::try_convert::<_, SVector<usize, 3>>(self.maximum - self.minimum).unwrap()
    }
    fn size(&self) -> usize {
        let axis = self.axis();
        axis[0] * axis[1] * axis[2]
    }
    fn get(&self, i: usize) -> &BlockInfo {
        let pos = nalgebra::convert::<_, SVector<isize, 3>>(self.delinearize(i)) + self.minimum;
        self.data
            .get(&pos)
            .expect("no block info entry found for position in blueprint")
    }
    fn get_mut(&mut self, i: usize) -> &mut BlockInfo {
        let pos = nalgebra::convert::<_, SVector<isize, 3>>(self.delinearize(i)) + self.minimum;
        if !self.data.contains_key(&pos) {
            self.data.insert(
                pos,
                BlockInfo {
                    block: Block::Air,
                    visible_mask: 0,
                },
            );
        }
        self.data.get_mut(&pos).unwrap()
    }
    fn contains(&self, i: usize) -> bool {
        let pos = nalgebra::convert::<_, SVector<isize, 3>>(self.delinearize(i)) + self.minimum;
        self.data.contains_key(&pos)
    }
}

#[derive(Debug)]
pub struct Chunk {
    data: [BlockInfo; CHUNK_SIZE],
}

impl Structure for Chunk {
    fn axis(&self) -> SVector<usize, 3> {
        SVector::<usize, 3>::new(CHUNK_AXIS, CHUNK_AXIS, CHUNK_AXIS)
    }
    fn size(&self) -> usize {
        CHUNK_SIZE
    }
    fn get(&self, i: usize) -> &BlockInfo {
        &self.data[i]
    }
    fn get_mut(&mut self, i: usize) -> &mut BlockInfo {
        &mut self.data[i]
    }
}

pub fn neighbors<F: FnMut(SVector<isize, 3>, NeighborDirection, usize, isize)>(
    pos: SVector<isize, 3>,
    mut f: F,
) {
    let mut dir_iter = NeighborDirection::iter();
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

fn inner_neighbors<F: FnMut(SVector<usize, 3>, NeighborDirection)>(
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

fn calc_block_visible_mask_inside_structure<S: Structure>(s: &S, i: usize) -> u8 {
    let mut mask = 0xFF;
    inner_neighbors(s.delinearize(i), s.axis(), |neighbor, dir| {
        let j = s.linearize(neighbor);
        if s.contains(j) && s.get(j).block.is_opaque() {
            mask &= !(1 << dir as u8);
        }
    });
    mask
}

pub fn calc_block_visible_mask_between_chunks(
    chunk: &mut Chunk,
    neighbor: &mut Chunk,
    dir: NeighborDirection,
    dimension: usize,
    normal: isize,
) {
    for u in 0..CHUNK_AXIS {
        for v in 0..CHUNK_AXIS {
            let mut my_block_position = SVector::<usize, 3>::new(0, 0, 0);
            let mut their_block_position = SVector::<usize, 3>::new(0, 0, 0);

            my_block_position[dimension] = if normal == 1 { CHUNK_AXIS - 1 } else { 0 };
            my_block_position[(dimension + 1) % 3] = u;
            my_block_position[(dimension + 2) % 3] = v;

            their_block_position[dimension] = if normal == 1 { 0 } else { CHUNK_AXIS - 1 };
            their_block_position[(dimension + 1) % 3] = u;
            their_block_position[(dimension + 2) % 3] = v;

            let BlockInfo {
                block: my_block,
                visible_mask: my_visible_mask,
            } = chunk.get_mut(chunk.linearize(my_block_position));
            let BlockInfo {
                block: their_block,
                visible_mask: their_visible_mask,
            } = neighbor.get_mut(neighbor.linearize(their_block_position));

            if my_block.is_opaque() {
                *their_visible_mask &= !(1 << dir.opposite() as u8);
            } else {
                *their_visible_mask |= 1 << dir.opposite() as u8;
            }

            if their_block.is_opaque() {
                *my_visible_mask &= !(1 << dir as u8);
            } else {
                *my_visible_mask |= 1 << dir as u8;
            }
        }
    }
}

pub fn gen_chunk(position: SVector<isize, 3>) -> Chunk {
    fn lerp3(a: f64, b: f64, c: f64, t: f64) -> f64 {
        use lerp::Lerp;
        return f64::lerp(f64::lerp(a, b, f64::min(t, 0.0) + 1.0), c, f64::max(t, 0.0));
    }
    let mut chunk = Chunk {
        data: [BlockInfo {
            block: Block::Air,
            visible_mask: 0xFF,
        }; CHUNK_SIZE],
    };

    let mut alpha = Fbm::<Simplex>::new(400);
    let mut beta = Fbm::<Simplex>::new(500);

    alpha.frequency = 0.0005;
    beta.frequency = 0.0005;

    let noise_scale = SVector::<isize, 3>::new(8, 8, 4);
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
                    world_gen_base(position + local_position, &alpha, &beta);
            }
        }
    }

    for i in 0..CHUNK_SIZE {
        use lerp::Lerp;
        let local_position = nalgebra::convert::<_, SVector<isize, 3>>(chunk.delinearize(i));

        let noise_interp = SVector::<f64, 3>::new(
            (local_position[0] as f64 / noise_scale[0] as f64).fract(),
            (local_position[1] as f64 / noise_scale[1] as f64).fract(),
            (local_position[2] as f64 / noise_scale[2] as f64).fract(),
        );

        let noise_position0 = SVector::<isize, 3>::new(
            local_position[0] / noise_scale[0],
            local_position[1] / noise_scale[1],
            local_position[2] / noise_scale[2],
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

        if density > 0.0 {
            chunk.get_mut(i).block = Block::Stone;
        }
    }

    for i in 0..CHUNK_SIZE {
        let mask = calc_block_visible_mask_inside_structure(&chunk, i);
        chunk.get_mut(i).visible_mask = mask;
    }
    chunk
}

pub fn cubic_block<F: Fn(&Block) -> Option<u32> + Copy>(
    info: &BlockInfo,
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

    for (i, dir) in NeighborDirection::iter().enumerate() {
        if ((info.visible_mask >> dir as u8) & 1) == 1 {
            let block_vertex_count = block_vertices.len() as u32;
            let mut tint = SVector::<f32, 4>::new(1.0, 1.0, 1.0, 1.0);
            block_vertices.extend([
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][0]] + position,
                    SVector::<f32, 2>::new(0.0, 0.0),
                    dir as u8,
                    (block_mapping)(&info.block).unwrap_or_default(),
                    tint,
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][1]] + position,
                    SVector::<f32, 2>::new(0.0, 1.0),
                    dir as u8,
                    (block_mapping)(&info.block).unwrap_or_default(),
                    tint,
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][2]] + position,
                    SVector::<f32, 2>::new(1.0, 1.0),
                    dir as u8,
                    (block_mapping)(&info.block).unwrap_or_default(),
                    tint,
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][3]] + position,
                    SVector::<f32, 2>::new(1.0, 0.0),
                    dir as u8,
                    (block_mapping)(&info.block).unwrap_or_default(),
                    tint,
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

pub fn gen_block_mesh<S: Structure, F: Fn(&Block) -> Option<u32> + Copy>(s: &S, block_mapping: F) -> (Vec<BlockVertex>, Vec<u32>) {
    let mut block_vertices = vec![];
    let mut block_indices = vec![];

    s.for_each(|i, info| {
        if info.block.is_opaque() {
            cubic_block(
                info,
                s.delinearize(i),
                block_mapping,
                &mut block_vertices,
                &mut block_indices,
            );
        }
    });
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
