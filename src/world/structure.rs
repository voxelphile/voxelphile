use std::{collections::HashMap, cell::{RefCell}};

use nalgebra::SVector;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::graphics::{vertex::BlockVertex, BlockMesh};

#[derive(Clone, Copy)]
pub enum Block {
    Empty,
    Full,
}

impl Block {
    fn is_opaque(&self) -> bool {
        !matches!(self, Block::Empty)
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

#[derive(Clone, Copy)]
pub struct BlockInfo {
    block: Block,
    visible_mask: u8,
}

pub const CHUNK_AXIS: usize = 32;
pub const CHUNK_SIZE: usize = CHUNK_AXIS * CHUNK_AXIS * CHUNK_AXIS;

pub trait Structure {
    fn axis(&self) -> SVector<usize, 3>;
    fn size(&self) -> usize;
    fn contains(&self, _: usize) -> bool { true }
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

pub struct Blueprint
{
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
        self.data.get(&pos).expect("no block info entry found for position in blueprint")
    }
    fn get_mut(&mut self, i: usize) -> &mut BlockInfo {
        let pos = nalgebra::convert::<_, SVector<isize, 3>>(self.delinearize(i)) + self.minimum;
        if !self.data.contains_key(&pos) {
            self.data.insert(pos, BlockInfo {
                block: Block::Empty,
                visible_mask: 0,
            });
        }
        self.data.get_mut(&pos).unwrap()
    }
    fn contains(&self, i: usize) -> bool {
        let pos = nalgebra::convert::<_, SVector<isize, 3>>(self.delinearize(i)) + self.minimum;
        self.data.contains_key(&pos)
    }
}

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

fn neighbors<F: FnMut(SVector<isize, 3>, NeighborDirection)>(pos: SVector<isize, 3>, mut f: F) {
    let mut dir_iter = NeighborDirection::iter();
    for d in 0..3 {
        for i in (-1..=1).step_by(2) {
            let dir = dir_iter.next().unwrap();

            let mut norm = SVector::<isize, 3>::new(0, 0, 0);
            norm[d] = i;

            let neighbor = pos + norm;

            (f)(neighbor, dir);
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
    neighbors(pos, |neighbor, dir| {
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

pub fn gen_chunk() -> Chunk {
    let mut chunk = Chunk {
        data: [BlockInfo {
            block: Block::Empty,
            visible_mask: 0,
        }; CHUNK_SIZE],
    };

    for i in 0..CHUNK_SIZE {
        let mask = calc_block_visible_mask_inside_structure(&chunk, i);
        chunk.get_mut(i).visible_mask = mask;
    }
    chunk
}

pub fn cubic_block(
    info: &BlockInfo,
    position: SVector<usize, 3>,
    block_vertices: &mut Vec<BlockVertex>,
    block_indices: &mut Vec<u32>,
) {
    const VERTEX_OFFSETS: [SVector<usize, 3>; 8] = [
        SVector::<usize, 3>::new(0, 0, 1),
        SVector::<usize, 3>::new(0, 1, 1),
        SVector::<usize, 3>::new(1, 1, 1),
        SVector::<usize, 3>::new(1, 0, 1),
        SVector::<usize, 3>::new(0, 0, 0),
        SVector::<usize, 3>::new(0, 1, 0),
        SVector::<usize, 3>::new(1, 1, 0),
        SVector::<usize, 3>::new(1, 0, 0),
    ];
    
    const VERTEX_SIDE_ORDER: [[usize; 4]; 6] = [
        [4,5,1,0],
        [3,2,6,7],
        [0,3,7,4],
        [5,6,2,1],
        [5,4,7,6],
        [0,1,2,3]
    ];
    
    for (i, dir) in NeighborDirection::iter().enumerate() {
        if (info.visible_mask >> dir as u8) & 1 != 0 {
            let block_vertex_count = block_vertices.len() as u32;
            block_vertices.extend([
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][0]] + position,
                    SVector::<f32, 2>::new(0.0, 0.0),
                    SVector::<f32, 4>::new(1.0, 1.0, 1.0, 1.0),
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][1]] + position,
                    SVector::<f32, 2>::new(0.0, 0.0),
                    SVector::<f32, 4>::new(1.0, 1.0, 1.0, 1.0),
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][2]] + position,
                    SVector::<f32, 2>::new(0.0, 0.0),
                    SVector::<f32, 4>::new(1.0, 1.0, 1.0, 1.0),
                ),
                BlockVertex::new(
                    VERTEX_OFFSETS[VERTEX_SIDE_ORDER[i][3]] + position,
                    SVector::<f32, 2>::new(0.0, 0.0),
                    SVector::<f32, 4>::new(1.0, 1.0, 1.0, 1.0),
                ),
            ]);
    
            block_indices.extend([
                block_vertex_count + 1,
                block_vertex_count + 3,
                block_vertex_count + 0,
                block_vertex_count + 1,
                block_vertex_count + 2,
                block_vertex_count + 3,
            ]);
        }
    }
}

pub fn gen_block_mesh<S: Structure>(s: &S) -> (Vec<BlockVertex>, Vec<u32>) {
    let mut block_vertices = vec![];
    let mut block_indices = vec![];

    s.for_each(|i, info| {
        cubic_block(
            info,
            s.delinearize(i),
            &mut block_vertices,
            &mut block_indices,
        );
    });
    (block_vertices, block_indices)
}
