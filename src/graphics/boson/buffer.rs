pub mod growable {
    use std::{
        collections::HashMap,
        mem, slice,
        sync::{Arc, Mutex},
    };

    use boson::prelude::*;

    use crate::graphics::boson::Boson;

    #[derive(Clone)]
    pub struct Internal {
        pub buffer: Option<Buffer>,
        pub size: usize,
    }

    pub struct Inner {
        device: Device,
        dummy: Buffer,
        buffer: Internal,
        old: Option<Internal>,
        counter: usize,
    }

    pub struct GrowableBuffer(Arc<Mutex<Inner>>);

    impl GrowableBuffer {
        pub fn buffer(&self) -> Option<Buffer> {
            self.0.lock().unwrap().buffer.buffer
        }
        pub fn size(&self) -> usize {
            self.0.lock().unwrap().buffer.size
        }
        pub fn new(device: Device, usage: BufferUsage) -> Self {
            let size = (1e+9 / 4.0) as usize;
            let buffer = Some(
                device
                    .create_buffer(BufferInfo {
                        size,
                        usage,
                        ..Default::default()
                    })
                    .unwrap(),
            );
            let dummy = device
                .create_buffer(BufferInfo {
                    size: 64,
                    usage: BufferUsage::STORAGE | BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                })
                .unwrap();
            Self(Arc::new(Mutex::new(Inner {
                device,
                dummy,
                buffer: Internal { buffer, size },
                old: None,
                counter: 0,
            })))
        }

        pub fn grow_to_atleast(&mut self, size: usize) {
            let size = size.next_power_of_two();
            let mut guard = self.0.lock().unwrap();
            if size > guard.buffer.size {
                if guard.old.is_none() && guard.buffer.buffer.is_some() {
                    guard.old = Some(guard.buffer.clone());
                    guard.buffer.buffer = None;
                } else if let Some(internal) = guard.old.take() {
                    guard.device.wait_idle();
                    guard
                        .device
                        .destroy_buffer(internal.buffer.unwrap())
                        .unwrap();
                    if guard.buffer.buffer.is_some() {
                        guard.old = Some(guard.buffer.clone());
                        guard.buffer.buffer = None;
                    }
                }
                guard.buffer.size = guard.buffer.size.max(size);
            }
            if guard.buffer.buffer.is_none() {
                dbg!(guard.buffer.size);
                guard.buffer.buffer = Some(
                    guard
                        .device
                        .create_buffer(BufferInfo {
                            size: guard.buffer.size,
                            usage: BufferUsage::STORAGE | BufferUsage::TRANSFER_DST,
                            ..Default::default()
                        })
                        .unwrap(),
                );
            }
        }

        pub(crate) fn task(&mut self, render_graph_builder: &mut RenderGraphBuilder<Boson>) {
            let inner_a = self.0.clone();
            let inner_b = self.0.clone();
            let inner_c = self.0.clone();

            if self.0.lock().unwrap().old.is_none() {
                return;
            }

            render_graph_builder.add(Task {
                resources: vec![
                    Resource::Buffer(
                        Box::new(move |frame| {
                            let guard = inner_b.lock().unwrap();

                            if let Some(old) = &guard.old {
                                old.buffer.unwrap()
                            } else {
                                guard.dummy
                            }
                        }),
                        BufferAccess::TransferRead,
                    ),
                    Resource::Buffer(
                        Box::new(move |boson| {
                            let mut guard = inner_a.lock().unwrap();

                            guard.buffer.buffer.unwrap()
                        }),
                        BufferAccess::TransferWrite,
                    ),
                ],
                task: move |boson, commands| {
                    let mut guard = inner_c.lock().unwrap();

                    if let Some(old) = &guard.old {
                        if guard.counter == 0 {
                            commands.copy_buffer_to_buffer(BufferCopy {
                                from: 0,
                                to: 1,
                                regions: vec![Region {
                                    src: 0,
                                    dst: 0,
                                    size: old.size,
                                }],
                            })?;
                        }
                    }

                    if guard.old.is_some() {
                        guard.counter += 1;
                        if guard.counter > 50 {
                            let internal = guard.old.take();

                            boson
                                .device
                                .destroy_buffer(internal.map(|x| x.buffer).flatten().unwrap())?;
                            guard.counter = 0;
                        }
                    }

                    Ok(())
                },
            });
        }
    }
}

pub mod staging {
    use std::{
        collections::HashMap,
        mem, slice,
        sync::{Arc, Mutex},
    };

    use boson::prelude::*;

    use crate::graphics::boson::Boson;

    #[derive(Clone, Debug)]
    pub enum Upload {
        Buffer {
            buffer: Buffer,
            dst: usize,
            data: Vec<u8>,
        },
        Image {
            image: Image,
            dst: (usize, usize, usize),
            size: (usize, usize, usize),
            data: Vec<u8>,
        },
    }

    pub struct Inner {
        buffer: Buffer,
        offset: usize,
        size: usize,
        uploads: Vec<Upload>,
    }

    #[derive(Clone)]
    pub struct StagingBuffer(Arc<Mutex<Inner>>);

    impl StagingBuffer {
        pub fn new(device: Device) -> Self {
            let size = (1e+9) as usize;
            let buffer = device
                .create_buffer(BufferInfo {
                    size,
                    memory: Memory::HOST_ACCESS,
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                })
                .unwrap();
            let offset = 0;

            Self(Arc::new(Mutex::new(Inner {
                buffer,
                offset,
                size,
                uploads: vec![],
            })))
        }
        pub fn upload_buffer<T: Clone + Copy>(&mut self, buffer: Buffer, dst: usize, data: &[T]) {
            let mut guard = self.0.lock().unwrap();
            let data = unsafe {
                slice::from_raw_parts(
                    data.as_ptr() as *const _ as *const u8,
                    mem::size_of::<T>() * data.len(),
                )
            }
            .to_vec();

            guard.uploads.push(Upload::Buffer { buffer, dst, data });
        }
        pub fn upload_image<T: Clone + Copy>(
            &mut self,
            image: Image,
            dst: (usize, usize, usize),
            size: (usize, usize, usize),
            data: &[T],
        ) {
            let mut guard = self.0.lock().unwrap();
            let data = unsafe {
                slice::from_raw_parts(
                    data.as_ptr() as *const _ as *const u8,
                    mem::size_of::<T>() * data.len(),
                )
            }
            .to_vec();

            guard.uploads.push(Upload::Image {
                image,
                dst,
                size,
                data,
            });
        }
        pub(crate) fn task(&mut self, render_graph_builder: &mut RenderGraphBuilder<Boson>) {
            let guard = self.0.lock().unwrap();
            let buffer = guard.buffer;

            let uploads = guard.uploads.clone();
            let mut buffer_mapping = HashMap::<Buffer, Vec<Upload>>::new();
            let mut image_uploads = vec![];

            for upload in uploads {
                if let Upload::Buffer { buffer, .. } = &upload {
                    buffer_mapping.entry(*buffer).or_default().push(upload);
                } else if let Upload::Image { .. } = &upload {
                    image_uploads.push(upload);
                }
            }

            let len = buffer_mapping.len();
            for (i, (buffer, uploads)) in buffer_mapping.into_iter().enumerate() {
                let from_buffer = guard.buffer;
                let to_buffer = buffer;
                let clear = i == len - 1;

                let offsets = Arc::new(Mutex::new(vec![]));

                let o1 = offsets.clone();
                let o2 = offsets.clone();

                let u1 = uploads.clone();
                let u2 = uploads.clone();

                render_graph_builder.add(Task {
                    resources: vec![Resource::Buffer(
                        Box::new(move |_| from_buffer),
                        BufferAccess::HostTransferWrite,
                    )],
                    task: move |boson, commands| {
                        let mut offset_guard = o1.lock().unwrap();

                        for upload in u1.clone() {
                            let Upload::Buffer { data, .. } = &upload else {
                                continue;
                            };
                            let offset = boson.staging.staging_offset_block(data.len());

                            offset_guard.push(offset);

                            commands.write_buffer(BufferWrite {
                                buffer: 0,
                                offset,
                                src: &data,
                            })?;
                        }
                        Ok(())
                    },
                });

                render_graph_builder.add(Task {
                    resources: vec![
                        Resource::Buffer(
                            Box::new(move |_| from_buffer),
                            BufferAccess::TransferRead,
                        ),
                        Resource::Buffer(Box::new(move |_| to_buffer), BufferAccess::TransferWrite),
                    ],
                    task: move |boson, commands| {
                        let mut offset_guard = o2.lock().unwrap();

                        let mut regions = vec![];
                        for (upload, offset) in
                            u2.clone().into_iter().zip(offset_guard.iter().cloned())
                        {
                            let Upload::Buffer { dst, data, .. } = &upload else {
                                continue;
                            };
                            regions.push(Region {
                                src: offset,
                                dst: *dst,
                                size: data.len(),
                            });
                        }
                        commands.copy_buffer_to_buffer(BufferCopy {
                            from: 0,
                            to: 1,
                            regions,
                        })?;

                        if clear {
                            boson.staging.clear_buffer_uploads();
                        }

                        Ok(())
                    },
                });
            }
            let len = image_uploads.len();
            for (i, upload) in image_uploads.into_iter().enumerate() {
                let Upload::Image { image, data, dst, size } = upload else {
                    continue;
                };
                let from_buffer = guard.buffer;
                let to_image = image;
                let clear = i == len - 1;

                let offsets = Arc::new(Mutex::new(0));

                let o1 = offsets.clone();
                let o2 = offsets.clone();

                let data_len = data.len();

                render_graph_builder.add(Task {
                    resources: vec![Resource::Buffer(
                        Box::new(move |_| from_buffer),
                        BufferAccess::HostTransferWrite,
                    )],
                    task: move |boson, commands| {
                        let mut offset_guard = o1.lock().unwrap();

                        let offset = boson.staging.staging_offset_block(data_len);

                        *offset_guard = offset;

                        commands.write_buffer(BufferWrite {
                            buffer: 0,
                            offset,
                            src: &data,
                        })?;
                        Ok(())
                    },
                });

                render_graph_builder.add(Task {
                    resources: vec![
                        Resource::Buffer(
                            Box::new(move |_| from_buffer),
                            BufferAccess::TransferRead,
                        ),
                        Resource::Image(
                            Box::new(move |_| to_image),
                            ImageAccess::TransferWrite,
                            ImageAspect::default(),
                        ),
                    ],
                    task: move |boson, commands| {
                        let mut offset_guard = o2.lock().unwrap();

                        commands.copy_buffer_to_image(ImageCopy {
                            from: 0,
                            to: 1,
                            src: *offset_guard,
                            dst,
                            size,
                        });

                        if clear {
                            boson.staging.clear_image_uploads();
                        }

                        Ok(())
                    },
                });
            }
        }
        fn clear_buffer_uploads(&mut self) {
            self.0
                .lock()
                .unwrap()
                .uploads
                .retain(|u| !matches!(u, Upload::Buffer { .. }));
        }
        fn clear_image_uploads(&mut self) {
            self.0
                .lock()
                .unwrap()
                .uploads
                .retain(|u| !matches!(u, Upload::Image { .. }));
        }
        fn staging_offset_block(&mut self, size: usize) -> usize {
            let mut guard = self.0.lock().unwrap();
            if guard.offset + size >= guard.size {
                guard.offset = 0;
                guard.offset
            } else {
                guard.offset += size;
                guard.offset - size
            }
        }
    }
}

pub mod indirect {
    use std::{collections::HashMap, mem, sync::MutexGuard};

    use boson::{
        prelude::{Buffer, BufferUsage, Device, Draw, DrawIndirectCommand},
        task::RenderGraphBuilder,
    };

    use crate::graphics::{
        boson::{indirect::BlockIndirectData, Boson},
        Indirect,
    };

    use super::growable::GrowableBuffer;

    pub trait IndirectProvider: Clone + Copy {}

    pub struct IndirectBuffer<T: IndirectProvider> {
        pub growable_buffer: GrowableBuffer,
        data: Vec<T>,
        mapping: HashMap<Indirect, usize>,
        rev_mapping: HashMap<usize, Indirect>,
        uploads: Vec<(usize, T)>,
        counter: Box<dyn Iterator<Item = Indirect> + 'static + Send + Sync>,
        dirty: bool,
    }

    impl<T: IndirectProvider> IndirectBuffer<T> {
        pub fn new(device: Device) -> Self {
            Self {
                growable_buffer: GrowableBuffer::new(
                    device,
                    BufferUsage::TRANSFER_DST | BufferUsage::INDIRECT | BufferUsage::STORAGE,
                ),
                data: vec![],
                uploads: vec![],
                mapping: HashMap::new(),
                rev_mapping: HashMap::new(),
                counter: Box::new((0..).into_iter().map(|x| Indirect(x))),
                dirty: false,
            }
        }
        pub fn add(&mut self, data: T) -> Indirect {
            let indirect = self.counter.next().unwrap();

            self.growable_buffer
                .grow_to_atleast((indirect.0 + 1) * mem::size_of::<T>());
            let idx = self.data.len();
            self.mapping.insert(indirect, idx);
            self.rev_mapping.insert(idx, indirect);
            self.upload(idx, data.clone());
            self.data.push(data);
            indirect
        }
        pub fn remove(&mut self, indirect: Indirect) {
            let idx = self.mapping[&indirect];
            let indirect = self.rev_mapping.remove(&(self.data.len() - 1)).unwrap();
            self.rev_mapping.insert(idx, indirect);
            self.mapping.insert(indirect, idx);
            self.data.swap_remove(idx);
            if let Some(data) = self.data.get(idx) {
                self.upload(idx, data.clone());
            }
        }
        fn upload(&mut self, idx: usize, upload: T) {
            self.dirty = true;
            self.uploads.push((idx, upload));
        }
        pub(crate) fn count(&self) -> usize {
            self.data.len()
        }
        pub(crate) fn buffer(&self) -> Buffer {
            self.growable_buffer.buffer().unwrap()
        }
        pub(crate) fn size(&self) -> usize {
            self.growable_buffer.size()
        }

        pub(crate) fn get(&mut self, indirect: Indirect) -> T {
            self.data[self.mapping[&indirect]].clone()
        }
        pub(crate) fn set(&mut self, indirect: Indirect, data: T) {
            self.upload(self.mapping[&indirect], data);
            self.data[self.mapping[&indirect]] = data;
        }
    }

    pub(crate) fn block_indirect_buffer_task(
        render_graph_builder: &mut RenderGraphBuilder<Boson>,
        boson: &mut Boson,
    ) {
        let mut uploads = vec![];
        let indirect_buffer = &mut boson.opaque_indirect;
        indirect_buffer.growable_buffer.task(render_graph_builder);

        for (idx, data) in &indirect_buffer.uploads {
            let Some(buffer) = indirect_buffer.growable_buffer.buffer() else {
                return;
            };
            uploads.push((
                buffer,
                idx * mem::size_of::<BlockIndirectData>(),
                [data.clone()],
            ));
        }
        indirect_buffer.uploads.clear();
        for (buffer, offset, data) in uploads {
            boson.staging.upload_buffer(buffer, offset, &data)
        }
    }
}
