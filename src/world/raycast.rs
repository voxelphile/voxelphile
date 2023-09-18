use std::ops::Rem;

use nalgebra::SVector;

use super::{
    structure::{Structure, CHUNK_AXIS},
    Chunk,  World, CHUNK_SIZE, block::Block,
};

#[derive(Clone, Debug)]
pub struct Descriptor {
    pub origin: SVector<f32, 3>,
    pub direction: SVector<f32, 3>,
    pub minimum: SVector<isize, 3>,
    pub maximum: SVector<isize, 3>,
    pub max_distance: f32,
}

#[derive(Clone, Debug)]
pub enum State {
    Traversal {
        position: SVector<isize, 3>,
        distance: f32,
        mask: SVector<bool, 3>,
        side_dist: SVector<f32, 3>,
        delta_dist: SVector<f32, 3>,
        ray_step: SVector<isize, 3>,
        step_count: usize,
    },
    VolumeNotReady,
    OutOfBounds,
    MaxDistReached,
    MaxStepReached,
    BlockFound(Block, Box<State>),
}

impl From<Descriptor> for State {
    fn from(mut desc: Descriptor) -> Self {
        desc.direction = desc.direction.normalize();
        State::Traversal {
            position: SVector::<isize, 3>::new(
                f32::floor(desc.origin.x) as isize,
                f32::floor(desc.origin.y) as isize,
                f32::floor(desc.origin.z) as isize,
            ),
            distance: 0.0,
            mask: SVector::<bool, 3>::new(false, false, false),
            side_dist: SVector::<f32, 3>::new(
                f32::signum(desc.direction.x)
                    * ((f32::floor(desc.origin.x) - desc.origin.x)
                        + (f32::signum(desc.direction.x) * 0.5)
                        + 0.5)
                    * (1.0 / f32::abs(desc.direction.x)),
                f32::signum(desc.direction.y)
                    * ((f32::floor(desc.origin.y) - desc.origin.y)
                        + (f32::signum(desc.direction.y) * 0.5)
                        + 0.5)
                    * (1.0 / f32::abs(desc.direction.y)),
                f32::signum(desc.direction.z)
                    * ((f32::floor(desc.origin.z) - desc.origin.z)
                        + (f32::signum(desc.direction.z) * 0.5)
                        + 0.5)
                    * (1.0 / f32::abs(desc.direction.z)),
            ),
            delta_dist: SVector::<f32, 3>::new(
                (1.0 / f32::abs(desc.direction.x)),
                (1.0 / f32::abs(desc.direction.y)),
                (1.0 / f32::abs(desc.direction.z)),
            ),
            ray_step: SVector::<isize, 3>::new(
                f32::signum(desc.direction.x) as isize,
                f32::signum(desc.direction.y) as isize,
                f32::signum(desc.direction.z) as isize,
            ),
            step_count: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ray {
    pub descriptor: Descriptor,
    pub state: State,
}

pub struct Hit {
    pub normal: SVector<isize, 3>,
    pub back_step: SVector<isize, 3>,
    pub position: SVector<isize, 3>,
    pub chunk_position: SVector<isize, 3>,
    pub local_position: SVector<usize, 3>,
    pub back_step_chunk_position: SVector<isize, 3>,
    pub back_step_local_position: SVector<usize, 3>,
    pub block: Block,
}

pub fn start(descriptor: Descriptor) -> Ray {
    Ray {
        state: descriptor.clone().into(),
        descriptor,
    }
}

pub fn drive<'a>(world: &World, ray: &'a mut Ray) -> &'a mut Ray {
    //checks
    {
        let State::Traversal {
            position,
            step_count,
            distance,
            ..
        } = ray.state else {
            return ray;
        };
        if step_count >= 100 {
            ray.state = State::MaxStepReached;
        }
        if distance > ray.descriptor.max_distance {
            ray.state = State::MaxDistReached;
        }
        {
            let i_chunk_size = SVector::<isize, 3>::new(
                CHUNK_AXIS as isize,
                CHUNK_AXIS as isize,
                CHUNK_AXIS as isize,
            );
            let translation_chunk = SVector::<isize, 3>::new(
                (position.x as isize).div_euclid(i_chunk_size.x),
                (position.y as isize).div_euclid(i_chunk_size.y),
                (position.z as isize).div_euclid(i_chunk_size.z),
            );
            /*
            let mut chunk = match world.chunks.get(&translation_chunk) {
                Some(ChunkState::Active { chunk, .. }) => chunk,
                _ => {
                    ray.state = State::OutOfBounds;
                    return ray;
                }
            };
            let mut local_position = SVector::<usize, 3>::new(
                (position.x as usize).rem_euclid(i_chunk_size.x as usize) as usize,
                (position.y as usize).rem_euclid(i_chunk_size.y as usize) as usize,
                (position.z as usize).rem_euclid(i_chunk_size.z as usize) as usize,
            );

            match chunk.get(chunk.linearize(local_position)).block {
                Block::Air => {}
                block => {
                    ray.state = State::BlockFound(block, Box::new(ray.state.clone()));
                }
                _ => {}
            }*/
        }
    }

    //step
    {
        let State::Traversal {
                position,
                distance,
                mask,
                side_dist,
                delta_dist,
                ray_step,
                step_count,
            } = &mut ray.state else {
                return ray;
            };

        mask.x = side_dist.x <= f32::min(side_dist.y, side_dist.z);
        mask.y = side_dist.y <= f32::min(side_dist.z, side_dist.x);
        mask.z = side_dist.z <= f32::min(side_dist.x, side_dist.y);
        let fmask = SVector::<f32, 3>::new(
            mask.x as i64 as f32,
            mask.y as i64 as f32,
            mask.z as i64 as f32,
        );
        let imask = SVector::<isize, 3>::new(mask.x as isize, mask.y as isize, mask.z as isize);
        *side_dist += SVector::<f32, 3>::new(
            fmask.x * delta_dist.x,
            fmask.y * delta_dist.y,
            fmask.z * delta_dist.z,
        );
        *position += SVector::<isize, 3>::new(
            imask.x * ray_step.x,
            imask.y * ray_step.y,
            imask.z * ray_step.z,
        );
        let a = SVector::<f32, 3>::new(
            fmask.x * (side_dist.x - delta_dist.x),
            fmask.y * (side_dist.y - delta_dist.y),
            fmask.z * (side_dist.z - delta_dist.z),
        );
        *distance = (a).dot(&a) / ray.descriptor.direction.dot(&ray.descriptor.direction);
        *step_count += 1;
    }

    ray
}

pub fn hit(ray: Ray) -> Option<Hit> {
    let State::BlockFound(block, prev_state) = ray.state else {
        None?
    };

    let State::Traversal { position, ray_step, mask,  .. } = *prev_state else {
        None?
    };

    let back_step = position
        - SVector::<isize, 3>::new(
            mask.x as isize * ray_step.x,
            mask.y as isize * ray_step.y,
            mask.z as isize * ray_step.z,
        );

    let i_chunk_size = SVector::<isize, 3>::new(
        CHUNK_AXIS as isize,
        CHUNK_AXIS as isize,
        CHUNK_AXIS as isize,
    );

    let chunk_position = SVector::<isize, 3>::new(
        (position.x as isize).div_euclid(i_chunk_size.x),
        (position.y as isize).div_euclid(i_chunk_size.y),
        (position.z as isize).div_euclid(i_chunk_size.z),
    );

    let local_position = SVector::<usize, 3>::new(
        (position.x as isize).rem_euclid(i_chunk_size.x) as usize,
        (position.y as isize).rem_euclid(i_chunk_size.y) as usize,
        (position.z as isize).rem_euclid(i_chunk_size.z) as usize,
    );
    let back_step_chunk_position = SVector::<isize, 3>::new(
        (back_step.x as isize).div_euclid(i_chunk_size.x),
        (back_step.y as isize).div_euclid(i_chunk_size.y),
        (back_step.z as isize).div_euclid(i_chunk_size.z),
    );

    let back_step_local_position = SVector::<usize, 3>::new(
        (back_step.x as isize).rem_euclid(i_chunk_size.x) as usize,
        (back_step.y as isize).rem_euclid(i_chunk_size.y) as usize,
        (back_step.z as isize).rem_euclid(i_chunk_size.z) as usize,
    );
    let normal = SVector::<f32, 3>::new(
        mask.x as isize as f32 * f32::signum(-ray.descriptor.direction.x),
        mask.y as isize as f32 * f32::signum(-ray.descriptor.direction.y),
        mask.z as isize as f32 * f32::signum(-ray.descriptor.direction.z),
    );
    let normal = SVector::<isize, 3>::new(normal.x as isize, normal.y as isize, normal.z as isize);

    Some(Hit {
        back_step,
        back_step_chunk_position,
        back_step_local_position,
        block,
        position,
        normal,
        local_position,
        chunk_position,
    })
}
