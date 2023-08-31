use boson::{device, prelude::*};
use std::{collections::HashMap, marker::PhantomData, mem, sync::Mutex};

use crate::graphics::Bucket;

use super::{buffer::growable::GrowableBuffer, Boson};

pub type Buckets = Vec<Bucket>;
pub type Count = usize;

pub struct Pool<T: Clone + Copy> {
    pub growable_buffer: GrowableBuffer,
    free: Vec<Bucket>,
    counter: Box<dyn Iterator<Item = Bucket> + 'static + Send + Sync>,
    max_count_per_bucket: usize,
    uploads: HashMap<usize, Vec<T>>,
    active: HashMap<Bucket, Count>,
}

impl<T: Clone + Copy> Pool<T> {
    pub(crate) fn max_count_per_bucket(&self) -> usize {
        self.max_count_per_bucket
    }
    pub(crate) fn count(&self) -> usize {
        self.active.len()
    }
    pub(crate) fn buffer(&self) -> Option<Buffer> {
        self.growable_buffer.buffer()
    }
    pub(crate) fn size(&self) -> usize {
        self.growable_buffer.size()
    }
    pub(crate) fn new(device: Device, usage: BufferUsage, max_count_per_bucket: usize) -> Self {
        let growable_buffer = GrowableBuffer::new(device, usage);
        Self {
            growable_buffer,
            free: vec![],
            counter: Box::new((0..).into_iter().map(|x| Bucket(x))),
            max_count_per_bucket,
            uploads: HashMap::new(),
            active: HashMap::new(),
        }
    }
    pub(crate) fn cmd(&self, bucket: Bucket) -> DrawIndirectCommand {
        DrawIndirectCommand {
            vertex_count: self.active[&bucket] as u32,
            instance_count: 1,
            first_vertex: (bucket.0 * self.max_count_per_bucket) as u32,
            first_instance: 0,
        }
    }
    pub(crate) fn unsection(&mut self, bucket: Bucket) {
        self.free.push(bucket);
    }

    pub(crate) fn section(&mut self, data: &[T]) -> Vec<Bucket> {
        if data.len() == 0 {
            return vec![];
        }

        let mut buckets = vec![];
        for cursor in (0..data.len()).step_by(self.max_count_per_bucket) {
            if self.free.len() == 0 {
                self.free.push(self.counter.next().unwrap());
            }

            let Some(bucket) = self.free.pop() else {
                panic!("this should never happen");
            };

            self.growable_buffer
                .grow_to_atleast((bucket.0 + 1) * mem::size_of::<T>() * self.max_count_per_bucket);

            buckets.push(bucket);

            let count_in_bucket = (data.len() - cursor).min(self.max_count_per_bucket);
            self.active.insert(bucket, count_in_bucket);
            self.uploads.insert(
                bucket.0 * mem::size_of::<T>() * self.max_count_per_bucket,
                data[cursor..(cursor + count_in_bucket)].to_vec(),
            );
        }
        buckets
    }
}

pub(crate) fn vertex_pool_upload(boson: &mut Boson) {
    let buffer = boson.block_vertices.growable_buffer.buffer();
    let Some(buffer) = buffer else {
        return;
    };
    for (offset, data) in boson.block_vertices.uploads.clone() {
        boson.staging.upload_buffer(buffer, offset, &data)
    }
    boson.block_vertices.uploads.clear();
}
pub(crate) fn index_pool_upload(boson: &mut Boson) {
    let buffer = boson.block_indices.growable_buffer.buffer();
    let Some(buffer) = buffer else {
        return;
    };
    for (offset, data) in boson.block_indices.uploads.clone() {
        boson.staging.upload_buffer(buffer, offset, &data)
    }
    boson.block_indices.uploads.clear();
}
