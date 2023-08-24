use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::window::Window;
use boson::{prelude::*, commands::RenderPassBeginInfo};

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
}

impl Boson {
    fn record(
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
                });

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
}

impl super::GraphicsInterface for Boson {
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

    fn resize(
        device: Device,
        old_swapchain: Option<Swapchain>,
        width: u32,
        height: u32,
    ) -> (Swapchain, RenderGraph<'static, Boson>) {
        let mut swapchain = device
            .create_swapchain(SwapchainInfo {
                width,
                height,
                present_mode: PresentMode::DoNotWaitForVBlank,
                image_usage: ImageUsage::COLOR,
                ..Default::default()
            })
            .expect("failed to create swapchain");

        let render_graph = Boson::record(device.clone(), swapchain, width, height);

        (swapchain, render_graph)
    }

    let mut swapchain = None;
    let mut render_graph = None;

    let winit::dpi::PhysicalSize { width, height } = window.inner_size();
    {
        let (s, r) = resize(device.clone(), swapchain, width, height);
        swapchain = Some(s);
        render_graph = Some(r);
    }

    let pipeline_compiler = device.create_pipeline_compiler(Default::default());


    let render_pass = device
    .create_render_pass(RenderPassInfo {
        color: vec![RenderPassAttachment {
            image: 0,
            load_op: LoadOp::Clear,
            format: device.presentation_format(swapchain.unwrap()).unwrap(),
            initial_layout: ImageLayout::ColorAttachmentOptimal,
            final_layout: ImageLayout::ColorAttachmentOptimal,
        }],
        depth: None,
        stencil_load_op: LoadOp::DontCare,
    })
    .unwrap();


    let framebuffers = device
    .get_swapchain_images(swapchain.unwrap())
    .unwrap()
    .into_iter()
    .map(|image| {
        device
            .create_framebuffer(FramebufferInfo {
                attachments: vec![image],
                width,
                height,
                render_pass: render_pass.clone(),
            })
            .unwrap()
    })
    .collect::<Vec<_>>();

    let acquire_semaphore = device
        .create_binary_semaphore(Default::default())
        .expect("failed to create semaphore");
    let present_semaphore = device
        .create_binary_semaphore(Default::default())
        .expect("failed to create semaphore");

    Self {
        context,
        device,
        swapchain,
        render_graph,
        render_pass,
        framebuffers,
        pipeline_compiler,
        acquire_semaphore,
        present_semaphore
    }
    }

    fn render(&mut self) {
        let mut render_graph = self.render_graph.take();
        render_graph.as_mut().unwrap().render(self);
        self.render_graph = render_graph;
    }
}