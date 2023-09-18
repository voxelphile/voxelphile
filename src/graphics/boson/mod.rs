mod atlas;
mod buffer;
mod indirect;
mod pool;

use std::{f32::consts::PI, fmt::Write, mem, collections::{HashMap, HashSet}};
use crate::world::{block::Block, RemoveChunkMesh, entity::{Dirty, Translation, Look, Observer}, structure::{Chunk, gen_block_mesh, Direction}, Active};

use super::{BlockMesh, Mesh};
use band::*;
use self::{
    atlas::Atlas,
    buffer::indirect::IndirectBuffer,
    buffer::{indirect::block_indirect_buffer_task, staging::StagingBuffer},
    indirect::{BlockIndirectData, EntityIndirectData},
    pool::{index_pool_task, vertex_pool_task, Pool},
};
use boson::{
    commands::RenderPassBeginInfo,
    pipeline::{Binding, BindingState},
    prelude::*,
};
use nalgebra::{Perspective3, Projective3, Quaternion, SMatrix, SVector, Unit, UnitQuaternion};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::Window;

use super::{vertex::{BlockVertex, EntityVertex}, Camera};

static UBER_VERT_SHADER: &'static [u8] = include_bytes!("../../../assets/uber.vert.spirv");
static UBER_FRAG_SHADER: &'static [u8] = include_bytes!("../../../assets/uber.frag.spirv");
static POSTFX_SHADER: &'static [u8] = include_bytes!("../../../assets/postfx.comp.spirv");
static BLUR_SHADER: &'static [u8] = include_bytes!("../../../assets/blur.comp.spirv");
static COMPOSITE_SHADER: &'static [u8] = include_bytes!("../../../assets/composite.comp.spirv");

#[derive(Clone, Copy)]
pub struct BlurPush {
    dir: u32,
}

pub struct Boson {
    context: Context,
    device: Device,
    swapchain: Option<Swapchain>,
    render_graph: Option<RenderGraph<'static, Self>>,
    pipeline_compiler: PipelineCompiler,
    render_pass: RenderPass,
    framebuffers: Vec<Framebuffer>,
    acquire_semaphore: BinarySemaphore,
    present_semaphore: BinarySemaphore,
    uber_pipeline: Pipeline,
    postfx_pipeline: Pipeline,
    blur_pipeline: Pipeline,
    composite_pipeline: Pipeline,
    global_buffer: Buffer,
    block_vertices: Pool<BlockVertex>,
    block_indices: Pool<u32>,
    entity_vertices: Pool<EntityVertex>,
    entity_indices: Pool<u32>,
    staging: StagingBuffer,
    opaque_indirect: IndirectBuffer<BlockIndirectData>,
    entity_indirect: IndirectBuffer<EntityIndirectData>,
    display_width: u32,
    display_height: u32,
    render_width: u32,
    render_height: u32,
    render_scale: u32,
    color: Image,
    ssao_output: Image,
    composite: Image,
    position: Image,
    normal: Image,
    depth: Image,
    noise: Image,
    ssao_kernel: Image,
    atlas: Atlas,
}

impl Boson {

    fn create_block_mesh(&mut self, info: super::BlockMesh) -> super::Mesh {
        let vertices = self.block_vertices.section(&info.vertices);
        let mut modified_indices = vec![];
        for cursor in 0..info.indices.len() {
            let index = info.indices[cursor];
            let relavent_bucket =
                vertices[index as usize / self.block_vertices.max_count_per_bucket()];
            let base_for_this_index =
                relavent_bucket.0 * self.block_vertices.max_count_per_bucket();

            modified_indices.push(
                base_for_this_index as u32
                    + index % self.block_vertices.max_count_per_bucket() as u32,
            );
        }
        let indices = self.block_indices.section(&modified_indices);
        let indirect_buffer = &mut self.opaque_indirect;
        let mut indirect = vec![];
        for bucket in &indices {
            let cmd = self.block_indices.cmd(*bucket);
            indirect.push(indirect_buffer.add(BlockIndirectData {
                cmd,
                position: SVector::<f32, 4>::new(
                    info.position.x,
                    info.position.y,
                    info.position.z,
                    1.0,
                ),
            }))
        }
        super::Mesh {
            vertices,
            indices,
            indirect,
        }
    }

    fn block_mapping(&self, block: Block, dir: Direction) -> Option<u32> {
        self.atlas.block_mapping(block, dir)
    }

    fn destroy_block_mesh(&mut self, mesh: super::Mesh){
        for bucket in mesh.vertices {
            self.block_vertices.unsection(bucket);
        }
        for bucket in mesh.indices {
            self.block_indices.unsection(bucket);
        }
        for indirect in mesh.indirect {
            self.opaque_indirect.remove(indirect);
        }
    }

    fn record(&mut self, device: Device, swapchain: Swapchain) -> RenderGraph<'static, Boson> {
        let mut render_graph_builder = device
            .create_render_graph::<'_, Boson>(RenderGraphInfo {
                swapchain,
                ..Default::default()
            })
            .expect("failed to create render graph builder");

        vertex_pool_task(&mut render_graph_builder, self);
        index_pool_task(&mut render_graph_builder, self);
        block_indirect_buffer_task(&mut render_graph_builder, self);
        self.staging.task(&mut render_graph_builder);

        render_graph_builder.add(Task {
            resources: vec![
                Resource::Buffer(
                    Box::new(move |boson| boson.opaque_indirect.buffer()),
                    BufferAccess::ShaderReadOnly,
                ),
                Resource::Buffer(
                    Box::new(move |boson| boson.global_buffer),
                    BufferAccess::ShaderReadOnly,
                ),
                Resource::Image(
                    Box::new(|boson| boson.color),
                    ImageAccess::ColorAttachment,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.position),
                    ImageAccess::ColorAttachment,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.normal),
                    ImageAccess::ColorAttachment,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.depth),
                    ImageAccess::DepthStencilAttachment,
                    ImageAspect::DEPTH,
                ),
                Resource::Image(
                    Box::new(|boson| {
                        boson
                            .device
                            .acquire_next_image(Acquire {
                                swapchain: boson.swapchain.unwrap(),
                                semaphore: Some(boson.acquire_semaphore),
                            })
                            .expect("failed to acquire next image")
                    }),
                    ImageAccess::ShaderReadOnly,
                    Default::default(),
                ),
            ],
            task: move |boson, commands| {
                let framebuffer = &boson.framebuffers[boson
                    .device
                    .get_current_frame_index(boson.swapchain.unwrap())
                    .unwrap()];

                commands.start_render_pass(RenderPassBeginInfo {
                    framebuffer,
                    render_pass: &boson.render_pass,
                    clear: vec![
                        Clear::Color(0.5, 0.6, 0.9, 1.0),
                        Clear::Color(0.0, 0.0, 0.0, 0.0),
                        Clear::Color(0.0, 0.0, 0.0, 0.0),
                        Clear::Depth(1.0),
                    ],
                    render_area: RenderArea {
                        x: 0,
                        y: 0,
                        width: boson.render_width,
                        height: boson.render_height,
                    },
                })?;

                commands.set_resolution((boson.render_width, boson.render_height), false)?;

                commands.set_pipeline(&boson.uber_pipeline)?;

                commands.write_bindings(
                    &boson.uber_pipeline,
                    vec![
                        WriteBinding::Buffer {
                            buffer: boson.block_vertices.buffer().unwrap(),
                            offset: 0,
                            range: boson.block_vertices.size(),
                        },
                        WriteBinding::Buffer {
                            buffer: boson.block_indices.buffer().unwrap(),
                            offset: 0,
                            range: boson.block_indices.size(),
                        },
                        WriteBinding::Buffer {
                            buffer: boson.opaque_indirect.buffer(),
                            offset: 0,
                            range: boson.opaque_indirect.size(),
                        },
                        WriteBinding::Buffer {
                            buffer: boson.global_buffer,
                            offset: 0,
                            range: 4096,
                        },
                        WriteBinding::Image(boson.atlas.image()),
                    ],
                )?;

                commands.draw_indirect(DrawIndirect {
                    buffer: 0,
                    offset: 0,
                    draw_count: boson.opaque_indirect.count(),
                    stride: mem::size_of::<BlockIndirectData>(),
                })?;

                commands.end_render_pass(&boson.render_pass);

                Ok(())
            },
        });

        render_graph_builder.add(Task {
            resources: vec![
                Resource::Image(
                    Box::new(|boson| boson.noise),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Buffer(
                    Box::new(move |boson| boson.global_buffer),
                    BufferAccess::ShaderReadOnly,
                ),
                Resource::Image(
                    Box::new(|boson| boson.ssao_kernel),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.position),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.normal),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.ssao_output),
                    ImageAccess::ComputeShaderWriteOnly,
                    Default::default(),
                ),
            ],
            task: move |boson, commands| {
                commands.set_pipeline(&boson.postfx_pipeline);
                commands.write_bindings(
                    &boson.postfx_pipeline,
                    vec![
                        WriteBinding::Buffer {
                            buffer: boson.global_buffer,
                            offset: 0,
                            range: 4096,
                        },
                        WriteBinding::Image(boson.noise),
                        WriteBinding::Image(boson.ssao_kernel),
                        WriteBinding::Image(boson.position),
                        WriteBinding::Image(boson.normal),
                        WriteBinding::Image(boson.ssao_output),
                    ],
                )?;
                commands.dispatch(
                    (boson.render_width as f32 / 8.0).ceil() as usize,
                    (boson.render_height as f32 / 8.0).ceil() as usize,
                    1,
                );
                Ok(())
            },
        });
        render_graph_builder.add(Task {
            resources: vec![
                Resource::Image(
                    Box::new(|boson| boson.normal),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Buffer(
                    Box::new(move |boson| boson.global_buffer),
                    BufferAccess::ShaderReadOnly,
                ),
                Resource::Image(
                    Box::new(|boson| boson.ssao_output),
                    ImageAccess::ComputeShaderReadWrite,
                    Default::default(),
                ),
            ],
            task: move |boson, commands| {
                commands.set_pipeline(&boson.blur_pipeline);
                commands.write_bindings(
                    &boson.blur_pipeline,
                    vec![
                        WriteBinding::Buffer {
                            buffer: boson.global_buffer,
                            offset: 0,
                            range: 4096,
                        },
                        WriteBinding::Image(boson.normal),
                        WriteBinding::Image(boson.ssao_output),
                    ],
                )?;
                commands.push_constant(PushConstant {
                    data: BlurPush { dir: 0 },
                    pipeline: &boson.blur_pipeline,
                });
                commands.dispatch(
                    (boson.render_width as f32 / 8.0).ceil() as usize,
                    (boson.render_height as f32 / 8.0).ceil() as usize,
                    1,
                );

                Ok(())
            },
        });
        render_graph_builder.add(Task {
            resources: vec![
                Resource::Image(
                    Box::new(|boson| boson.normal),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Buffer(
                    Box::new(move |boson| boson.global_buffer),
                    BufferAccess::ShaderReadOnly,
                ),
                Resource::Image(
                    Box::new(|boson| boson.ssao_output),
                    ImageAccess::ComputeShaderReadWrite,
                    Default::default(),
                ),
            ],
            task: move |boson, commands| {
                commands.set_pipeline(&boson.blur_pipeline);
                commands.write_bindings(
                    &boson.blur_pipeline,
                    vec![
                        WriteBinding::Buffer {
                            buffer: boson.global_buffer,
                            offset: 0,
                            range: 4096,
                        },
                        WriteBinding::Image(boson.normal),
                        WriteBinding::Image(boson.ssao_output),
                    ],
                )?;
                commands.push_constant(PushConstant {
                    data: BlurPush { dir: 1 },
                    pipeline: &boson.blur_pipeline,
                });
                commands.dispatch(
                    (boson.render_width as f32 / 8.0).ceil() as usize,
                    (boson.render_height as f32 / 8.0).ceil() as usize,
                    1,
                );

                Ok(())
            },
        });
        render_graph_builder.add(Task {
            resources: vec![
                Resource::Image(
                    Box::new(|boson| boson.ssao_output),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.color),
                    ImageAccess::ComputeShaderReadOnly,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| boson.composite),
                    ImageAccess::ComputeShaderWriteOnly,
                    Default::default(),
                ),
            ],
            task: move |boson, commands| {
                commands.set_pipeline(&boson.composite_pipeline);
                commands.write_bindings(
                    &boson.composite_pipeline,
                    vec![
                        WriteBinding::Buffer {
                            buffer: boson.global_buffer,
                            offset: 0,
                            range: 4096,
                        },
                        WriteBinding::Image(boson.ssao_output),
                        WriteBinding::Image(boson.color),
                        WriteBinding::Image(boson.composite),
                    ],
                )?;
                commands.dispatch(
                    (boson.render_width as f32 / 8.0).ceil() as usize,
                    (boson.render_height as f32 / 8.0).ceil() as usize,
                    1,
                );
                Ok(())
            },
        });
        render_graph_builder.add(Task {
            resources: vec![
                Resource::Image(
                    Box::new(|boson| boson.composite),
                    ImageAccess::TransferRead,
                    Default::default(),
                ),
                Resource::Image(
                    Box::new(|boson| {
                        boson
                            .device
                            .acquire_next_image(Acquire {
                                swapchain: boson.swapchain.unwrap(),
                                semaphore: Some(boson.acquire_semaphore),
                            })
                            .expect("failed to acquire next image")
                    }),
                    ImageAccess::TransferWrite,
                    Default::default(),
                ),
            ],
            task: move |boson, commands| {
                commands.blit_image(BlitImage {
                    from: 0,
                    to: 1,
                    src: (boson.render_width as _, boson.render_height as _, 1),
                    dst: (boson.display_width as _, boson.display_height as _, 1),
                });
                Ok(())
            },
        });

        render_graph_builder.add(Task {
            resources: vec![Resource::Image(
                Box::new(|boson| {
                    boson
                        .device
                        .acquire_next_image(Acquire {
                            swapchain: boson.swapchain.unwrap(),
                            semaphore: Some(boson.acquire_semaphore),
                        })
                        .expect("failed to acquire next image")
                }),
                ImageAccess::Present,
                Default::default(),
            )],
            task: |boson, commands| {
                commands.submit(Submit {
                    wait_semaphore: Some(boson.acquire_semaphore),
                    signal_semaphore: Some(boson.present_semaphore),
                })?;

                commands.present(Present {
                    wait_semaphore: boson.present_semaphore,
                })?;
                Ok(())
            },
        });

        render_graph_builder
            .complete()
            .expect("failed to create render graph")
    }
    fn commit_resize(
        &mut self,
        device: Device,
        render_pass: RenderPass,
        old_swapchain: Option<Swapchain>,
    ) -> (Swapchain, RenderGraph<'static, Boson>, Vec<Framebuffer>) {
        self.render_width = self.display_width / self.render_scale;
        self.render_height = self.display_height / self.render_scale;
        let swapchain = device
            .create_swapchain(SwapchainInfo {
                width: self.display_width,
                height: self.display_height,
                present_mode: PresentMode::DoNotWaitForVBlank,
                old_swapchain,
                image_usage: ImageUsage::TRANSFER_DST,
                ..Default::default()
            })
            .expect("failed to create swapchain");

        let framebuffers = device
            .get_swapchain_images(swapchain)
            .unwrap()
            .into_iter()
            .map(|image| {
                device
                    .create_framebuffer(FramebufferInfo {
                        attachments: vec![self.color, self.position, self.normal, self.depth],
                        width: self.render_width,
                        height: self.render_height,
                        render_pass: render_pass.clone(),
                    })
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let render_graph = self.record(device.clone(), swapchain);

        (swapchain, render_graph, framebuffers)
    }
}

impl super::GraphicsInterface for Boson {
    fn resize(&mut self, width: u32, height: u32) {
        if self.display_width == width && self.display_height == height {
            return;
        }
        self.device.wait_idle();

        self.display_width = width;
        self.display_height = height;

        let old_swapchain = self.swapchain.take();

        let (s, r, f) =
            self.commit_resize(self.device.clone(), self.render_pass.clone(), old_swapchain);

        self.swapchain = Some(s);
        self.render_graph = Some(r);
        self.framebuffers = f;
    }
    fn init(window: &Window) -> Self {
        let context = Context::new(ContextInfo {
            enable_validation: false,
            application_name: "Xenotech",
            ..Default::default()
        })
        .expect("failed to create context");

        let mut device = context
            .create_device(DeviceInfo {
                display: window.raw_display_handle(),
                window: window.raw_window_handle(),
                ..Default::default()
            })
            .expect("failed to create device");

        let depth = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(4096, 4096),
                usage: ImageUsage::DEPTH_STENCIL,
                format: Format::D32Sfloat,
                debug_name: "depth",
            })
            .unwrap();

        let color = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(4096, 4096),
                usage: ImageUsage::COLOR | ImageUsage::TRANSFER_SRC,
                format: Format::Rgba32Sfloat,
                debug_name: "color",
            })
            .unwrap();
        let composite = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(4096, 4096),
                usage: ImageUsage::COLOR | ImageUsage::TRANSFER_SRC,
                format: Format::Rgba32Sfloat,
                debug_name: "composite",
            })
            .unwrap();
        let position = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(4096, 4096),
                usage: ImageUsage::COLOR | ImageUsage::TRANSFER_SRC,
                format: Format::Rgba32Sfloat,
                debug_name: "position",
            })
            .unwrap();
        let normal = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(4096, 4096),
                usage: ImageUsage::COLOR | ImageUsage::TRANSFER_SRC,
                format: Format::Rgba32Sfloat,
                debug_name: "normal",
            })
            .unwrap();
        let ssao_output = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(4096, 4096),
                usage: ImageUsage::COLOR | ImageUsage::TRANSFER_SRC,
                format: Format::Rgba32Sfloat,
                debug_name: "color",
            })
            .unwrap();

        let render_pass = device
            .create_render_pass(RenderPassInfo {
                color: vec![
                    RenderPassAttachment {
                        image: 0,
                        load_op: LoadOp::Clear,
                        format: Format::Rgba32Sfloat,
                        initial_layout: ImageLayout::ColorAttachmentOptimal,
                        final_layout: ImageLayout::ColorAttachmentOptimal,
                    },
                    RenderPassAttachment {
                        image: 1,
                        load_op: LoadOp::Clear,
                        format: Format::Rgba32Sfloat,
                        initial_layout: ImageLayout::ColorAttachmentOptimal,
                        final_layout: ImageLayout::ColorAttachmentOptimal,
                    },
                    RenderPassAttachment {
                        image: 2,
                        load_op: LoadOp::Clear,
                        format: Format::Rgba32Sfloat,
                        initial_layout: ImageLayout::ColorAttachmentOptimal,
                        final_layout: ImageLayout::ColorAttachmentOptimal,
                    },
                ],
                depth: Some(RenderPassAttachment {
                    image: 3,
                    load_op: LoadOp::Clear,
                    format: Format::D32Sfloat,
                    initial_layout: ImageLayout::DepthStencilAttachmentOptimal,
                    final_layout: ImageLayout::DepthStencilAttachmentOptimal,
                }),
                stencil_load_op: LoadOp::DontCare,
            })
            .unwrap();

        let pipeline_compiler = device.create_pipeline_compiler(Default::default());

        let uber_pipeline = pipeline_compiler
            .create_graphics_pipeline(GraphicsPipelineInfo {
                shaders: vec![
                    Shader {
                        ty: ShaderType::Vertex,
                        source: UBER_VERT_SHADER.to_vec(),
                        defines: vec![],
                    },
                    Shader {
                        ty: ShaderType::Fragment,
                        source: UBER_FRAG_SHADER.to_vec(),
                        defines: vec![],
                    },
                ],
                color: vec![
                    Color {
                        format: Format::Rgba32Sfloat,
                        ..Default::default()
                    },
                    Color {
                        format: Format::Rgba32Sfloat,
                        ..Default::default()
                    },
                    Color {
                        format: Format::Rgba32Sfloat,
                        ..Default::default()
                    },
                ],
                depth: Some(Depth {
                    write: true,
                    compare: CompareOp::LessOrEqual,
                    format: Format::D32Sfloat,
                    ..Default::default()
                }),
                render_pass: Some(render_pass.clone()),
                binding: BindingState::Binding(vec![
                    Binding::Buffer,
                    Binding::Buffer,
                    Binding::Buffer,
                    Binding::Buffer,
                    Binding::Image,
                ]),
                raster: Raster {
                    face_cull: FaceCull::BACK,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();

        let postfx_pipeline = pipeline_compiler
            .create_compute_pipeline(ComputePipelineInfo {
                shader: Shader {
                    ty: ShaderType::Compute,
                    source: POSTFX_SHADER.to_vec(),
                    defines: vec![],
                },
                binding: BindingState::Binding(vec![
                    Binding::Buffer,
                    Binding::Image,
                    Binding::Image,
                    Binding::Image,
                    Binding::Image,
                    Binding::Image,
                ]),
                debug_name: "postfx".to_string(),
                ..Default::default()
            })
            .unwrap();
        let blur_pipeline = pipeline_compiler
            .create_compute_pipeline(ComputePipelineInfo {
                shader: Shader {
                    ty: ShaderType::Compute,
                    source: BLUR_SHADER.to_vec(),
                    defines: vec![],
                },
                binding: BindingState::Binding(vec![
                    Binding::Buffer,
                    Binding::Image,
                    Binding::Image,
                ]),
                push_constant_size: std::mem::size_of::<BlurPush>(),
                debug_name: "blur".to_string(),
                ..Default::default()
            })
            .unwrap();
        let composite_pipeline = pipeline_compiler
            .create_compute_pipeline(ComputePipelineInfo {
                shader: Shader {
                    ty: ShaderType::Compute,
                    source: COMPOSITE_SHADER.to_vec(),
                    defines: vec![],
                },
                binding: BindingState::Binding(vec![
                    Binding::Buffer,
                    Binding::Image,
                    Binding::Image,
                    Binding::Image,
                ]),
                debug_name: "composite".to_string(),
                ..Default::default()
            })
            .unwrap();

        let framebuffers = vec![];

        let acquire_semaphore = device
            .create_binary_semaphore(Default::default())
            .expect("failed to create semaphore");
        let present_semaphore = device
            .create_binary_semaphore(Default::default())
            .expect("failed to create semaphore");

        let global_buffer = device
            .create_buffer(BufferInfo {
                size: 4096,
                usage: BufferUsage::TRANSFER_DST | BufferUsage::STORAGE,
                debug_name: "global",
                ..Default::default()
            })
            .unwrap();

        let block_vertices = Pool::new(
            device.clone(),
            BufferUsage::TRANSFER_DST | BufferUsage::TRANSFER_SRC | BufferUsage::STORAGE,
            600,
        );
        let block_indices = Pool::new(
            device.clone(),
            BufferUsage::TRANSFER_DST | BufferUsage::TRANSFER_SRC | BufferUsage::STORAGE,
            6000,
        );
        let entity_vertices = Pool::new(
            device.clone(),
            BufferUsage::TRANSFER_DST | BufferUsage::TRANSFER_SRC | BufferUsage::STORAGE,
            24,
        );
        let entity_indices = Pool::new(
            device.clone(),
            BufferUsage::TRANSFER_DST | BufferUsage::TRANSFER_SRC | BufferUsage::STORAGE,
            36,
        );
        let mut staging = StagingBuffer::new(device.clone());
        let opaque_indirect = IndirectBuffer::new(device.clone());
        let entity_indirect = IndirectBuffer::new(device.clone());
        let winit::dpi::PhysicalSize { width, height } = window.inner_size();

        let noise = device
            .create_image(ImageInfo {
                extent: ImageExtent::TwoDim(256, 256),
                usage: ImageUsage::TRANSFER_DST,
                format: Format::Rgba32Uint,
                debug_name: "noise",
            })
            .unwrap();
        let ssao_kernel = device
            .create_image(ImageInfo {
                extent: ImageExtent::OneDim(8),
                usage: ImageUsage::TRANSFER_DST,
                format: Format::Rgba32Sfloat,
                debug_name: "ssao_kernel",
            })
            .unwrap();

        use rand::prelude::*;
        let mut rng = rand::thread_rng();
        {
            let mut values = Vec::<u32>::new();
            for _ in 0..4 * 256 * 256 {
                values.push(rng.gen::<u32>());
            }
            staging.upload_image(noise, (0, 0, 0), (256, 256, 1), &values);
        }
        {
            let mut samples = Vec::<f32>::new();
            for i in 0..8 {
                let mut sample = SVector::<f32, 3>::new(
                    rng.gen_range::<f32, _, _>(0.0, 1.0) * 2.0 - 1.0,
                    rng.gen_range::<f32, _, _>(0.0, 1.0) * 2.0 - 1.0,
                    rng.gen_range::<f32, _, _>(0.0, 1.0),
                );
                sample = sample.normalize();
                sample *= rng.gen_range::<f32, _, _>(0.0, 1.0);
                let mut scale = i as f32 / 8.0;
                use lerp::Lerp;
                scale = Lerp::lerp(0.1, 1.0, scale * scale);
                sample *= scale;
                samples.push(sample.x);
                samples.push(sample.y);
                samples.push(sample.z);
                samples.push(0.0);
            }
            staging.upload_image(ssao_kernel, (0, 0, 0), (8, 1, 1), &samples);
        }

        let atlas = Atlas::load(&device, &mut staging);

        let render_scale = 2;

        let mut boson = Self {
            swapchain: None,
            render_graph: None,
            context,
            device,
            render_pass,
            framebuffers,
            pipeline_compiler,
            acquire_semaphore,
            present_semaphore,
            uber_pipeline,
            global_buffer,
            block_vertices,
            block_indices,
            staging,
            opaque_indirect,
            display_width: width,
            display_height: height,
            render_scale,
            render_width: 0,
            render_height: 0,
            depth,
            atlas,
            noise,
            ssao_kernel,
            color,
            composite,
            ssao_output,
            position,
            normal,
            postfx_pipeline,
            composite_pipeline,
            blur_pipeline,
            entity_vertices,
            entity_indices,
            entity_indirect,
        };

        {
            let (s, r, f) =
                boson.commit_resize(boson.device.clone(), boson.render_pass.clone(), None);
            boson.swapchain = Some(s);
            boson.render_graph = Some(r);
            boson.framebuffers = f;
        }

        boson
    }

    fn render(&mut self, registry: &mut Registry) {
        let Some((observer, Translation(translation), Look(look))) = <(&Observer, &Translation, &Look)>::query(registry).next() else {
            return;
        };

        let mut remove_dirty = HashSet::new();
        let mut remove_mesh = HashSet::new();
        let mut add_mesh = HashMap::new();

        for (entity, chunk, Translation(position), _, _, mesh) in <(Entity, &Chunk, &Translation, &Active, &Dirty, Option<&Mesh>)>::query(registry) {
            let (vertices, indices) =
                gen_block_mesh(chunk, |block, dir| self.block_mapping(block, dir));
            if let Some(_) = mesh {
                remove_mesh.insert(entity);
            }
            remove_dirty.insert(entity);
            let position = *position;
            add_mesh.insert(entity, BlockMesh {
                vertices,
                indices,
                position,
            });
        }

        for (entity, _, _, _) in <(Entity, &Chunk, &Mesh, &RemoveChunkMesh)>::query(registry) {
            remove_mesh.insert(entity);
        } 

        for entity in remove_mesh {
            self.destroy_block_mesh(registry.remove::<Mesh>(entity).unwrap());
            if let Some(_) = registry.get::<RemoveChunkMesh>(entity) {
                registry.remove::<RemoveChunkMesh>(entity);
            }
        }

        for (entity, mesh) in add_mesh {
            registry.insert(entity, self.create_block_mesh(mesh));
        }

        for entity in remove_dirty {
            registry.remove::<Dirty>(entity);
        }

        {
            let clip = SMatrix::<f32, 4, 4>::new(
                1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
            );
            let trans = (SMatrix::<f32, 4, 4>::new_translation(&translation)
                * (UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
                    look.x,
                ) * UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(SVector::<f32, 3>::new(1.0, 0.0, 0.0)),
                    look.y,
                ))
                .to_homogeneous());
            self.staging.upload_buffer(
                self.global_buffer,
                0,
                &[Camera {
                    proj: clip
                        * Perspective3::new(
                            self.render_width as f32 / self.render_height as f32,
                            PI / 2.0,
                            0.1,
                            1000.0,
                        )
                        .into_inner(),
                    trans,
                    view: trans.try_inverse().unwrap(),
                    resolution: SVector::<u32, 4>::new(self.render_width, self.render_height, 0, 0),
                }],
            );
        }

        self.render_graph = Some(self.record(self.device.clone(), self.swapchain.clone().unwrap()));

        let mut render_graph = self.render_graph.take();
        render_graph.as_mut().unwrap().render(self);
        self.render_graph = render_graph;
    }
}
