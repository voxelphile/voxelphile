mod buffer;
mod indirect;
mod pool;

use std::{f32::consts::PI, mem};

use self::{
    buffer::indirect::IndirectBuffer,
    buffer::{indirect::indirect_buffer_upload, staging::StagingBuffer},
    indirect::IndirectData,
    pool::{index_pool_upload, vertex_pool_upload, Pool},
};
use boson::{
    commands::RenderPassBeginInfo,
    pipeline::{Binding, BindingState},
    prelude::*,
};
use nalgebra::{Perspective3, Projective3, Quaternion, SMatrix, SVector, Unit, UnitQuaternion};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::Window;

use super::{vertex::BlockVertex, Camera};

static UBER_VERT_SHADER: &'static [u8] = include_bytes!("../../../assets/uber.vert.spirv");
static UBER_FRAG_SHADER: &'static [u8] = include_bytes!("../../../assets/uber.frag.spirv");

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
    global_buffer: Buffer,
    block_vertices: Pool<BlockVertex>,
    block_indices: Pool<u32>,
    staging: StagingBuffer,
    opaque_indirect: IndirectBuffer<IndirectData>,
    width: u32,
    height: u32,
}

impl Boson {
    fn record(
        &mut self,
        device: Device,
        swapchain: Swapchain,
        width: u32,
        height: u32,
    ) -> RenderGraph<'static, Boson> {
        let mut render_graph_builder = device
            .create_render_graph::<'_, Boson>(RenderGraphInfo {
                swapchain,
                ..Default::default()
            })
            .expect("failed to create render graph builder");

        self.block_vertices
            .growable_buffer
            .task(&mut render_graph_builder);
        self.block_indices
            .growable_buffer
            .task(&mut render_graph_builder);
        self.opaque_indirect
            .growable_buffer
            .task(&mut render_graph_builder);

        self.staging.task(&mut render_graph_builder);

        render_graph_builder.add(Task {
            resources: vec![
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
                    ImageAccess::ColorAttachment,
                    Default::default(),
                ),
                Resource::Buffer(
                    Box::new(move |boson| boson.opaque_indirect.buffer()),
                    BufferAccess::ShaderReadOnly,
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
                    clear: vec![Clear::Color(0.5, 0.6, 0.9, 1.0), Clear::Depth(1.0)],
                    render_area: RenderArea {
                        x: 0,
                        y: 0,
                        width,
                        height,
                    },
                })?;

                commands.set_resolution((width, height), false)?;

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
                    ],
                )?;

                commands.set_pipeline(&boson.uber_pipeline)?;

                commands.draw_indirect(DrawIndirect {
                    buffer: 1,
                    offset: 0,
                    draw_count: boson.opaque_indirect.count(),
                    stride: mem::size_of::<IndirectData>(),
                })?;

                commands.end_render_pass(&boson.render_pass);

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
                ImageAccess::ColorAttachment,
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
        let swapchain = device
            .create_swapchain(SwapchainInfo {
                width: self.width,
                height: self.height,
                present_mode: PresentMode::DoNotWaitForVBlank,
                old_swapchain,
                image_usage: ImageUsage::COLOR,
                ..Default::default()
            })
            .expect("failed to create swapchain");

        let render_graph = self.record(device.clone(), swapchain, self.width, self.height);

        let framebuffers = device
            .get_swapchain_images(swapchain)
            .unwrap()
            .into_iter()
            .map(|image| {
                device
                    .create_framebuffer(FramebufferInfo {
                        attachments: vec![image],
                        width: self.width,
                        height: self.height,
                        render_pass: render_pass.clone(),
                    })
                    .unwrap()
            })
            .collect::<Vec<_>>();

        (swapchain, render_graph, framebuffers)
    }
}

impl super::GraphicsInterface for Boson {
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;

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

        let render_pass = device
            .create_render_pass(RenderPassInfo {
                color: vec![RenderPassAttachment {
                    image: 0,
                    load_op: LoadOp::Clear,
                    format: Format::Bgra8Srgb,
                    initial_layout: ImageLayout::ColorAttachmentOptimal,
                    final_layout: ImageLayout::ColorAttachmentOptimal,
                }],
                depth: None,
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
                color: vec![Color {
                    format: Format::Bgra8Srgb,
                    ..Default::default()
                }],
                depth: None,
                render_pass: Some(render_pass.clone()),
                binding: BindingState::Binding(vec![
                    Binding::Buffer,
                    Binding::Buffer,
                    Binding::Buffer,
                    Binding::Buffer,
                ]),
                raster: Raster {
                    face_cull: FaceCull::BACK,
                    polygon_mode: PolygonMode::Line,
                    front_face: FrontFace::Clockwise,
                    ..Default::default()
                },
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
                memory: Memory::empty(),
                usage: BufferUsage::TRANSFER_DST | BufferUsage::STORAGE,
                debug_name: "global",
            })
            .unwrap();

        let block_vertices = Pool::new(
            device.clone(),
            BufferUsage::TRANSFER_DST | BufferUsage::STORAGE,
            3600,
        );
        let block_indices = Pool::new(
            device.clone(),
            BufferUsage::TRANSFER_DST | BufferUsage::STORAGE,
            3000,
        );
        let staging = StagingBuffer::new(device.clone());
        let opaque_indirect = IndirectBuffer::new(device.clone());
        let winit::dpi::PhysicalSize { width, height } = window.inner_size();

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
            width,
            height,
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

    fn render(&mut self, look: SVector<f32, 2>, translation: SVector<f32, 3>) {
        vertex_pool_upload(self);
        index_pool_upload(self);
        indirect_buffer_upload(self);

        let clip = SMatrix::<f32, 4, 4>::new(
            1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 1.0,
        );

        self.staging.upload_buffer(
            self.global_buffer,
            0,
            &[Camera {
                proj: clip
                    * Perspective3::new(
                        self.width as f32 / self.height as f32,
                        PI / 2.0,
                        0.1,
                        100.0,
                    )
                    .into_inner(),
                view: (SMatrix::<f32, 4, 4>::new_translation(&translation)
                    * (UnitQuaternion::from_axis_angle(
                        &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
                        look.x,
                    ) * UnitQuaternion::from_axis_angle(
                        &Unit::new_normalize(SVector::<f32, 3>::new(1.0, 0.0, 0.0)),
                        look.y,
                    ))
                    .to_homogeneous())
                .try_inverse()
                .unwrap(),
            }],
        );

        self.render_graph = Some(self.record(
            self.device.clone(),
            self.swapchain.clone().unwrap(),
            self.width,
            self.height,
        ));

        let mut render_graph = self.render_graph.take();
        render_graph.as_mut().unwrap().render(self);
        self.render_graph = render_graph;
    }

    fn create_block_mesh(&mut self, info: super::BlockMesh<'_>) -> super::Mesh {
        let vertices = self.block_vertices.section(info.vertices);
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

            indirect.push(indirect_buffer.add(IndirectData {
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
}
