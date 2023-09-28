#![feature(let_chains, hash_drain_filter)]
mod graphics;
pub mod input;
pub mod net;
pub mod util;
mod world;
use band::{Entity, Registry};
use core::panic;
use graphics::{vertex::BlockVertex, *};
use input::Input;
use nalgebra::SVector;
use net::*;
use std::{
    f32::consts::PI,
    ops,
    time::{self, SystemTime},
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, MouseButton, VirtualKeyCode, WindowEvent},
    platform::run_return::EventLoopExtRunReturn,
    window::{CursorGrabMode, WindowBuilder},
};
use world::entity::{Loader, Speed};
use world::{
    structure::{gen_block_mesh, CHUNK_AXIS},
    ClientWorld,
};

use crate::{
    input::Inputs,
    world::{
        block::Block,
        entity::{Change, Look, Main, Observer, Target, Translation},
        ServerWorld,
    },
};

pub struct EventLoop(pub winit::event_loop::EventLoop<()>);

//SAFETY: Who knows, its android.
unsafe impl Send for EventLoop {}
unsafe impl Sync for EventLoop {}

impl ops::Deref for EventLoop {
    type Target = winit::event_loop::EventLoop<()>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ops::DerefMut for EventLoop {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub static mut EVENT_LOOP: Option<EventLoop> = None;

#[no_mangle]
#[cfg(target_os = "android")]
fn android_main(app: AndroidApp) {
    if unsafe { EVENT_LOOP.is_none() } {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        unsafe {
            EVENT_LOOP = Some(EventLoop(
                EventLoopBuilder::new().with_android_app(app).build(),
            ));
        }
    }
    main();
}

const FIXED_TIME: f32 = 1.0 / 20.0;

pub fn main() {
    #[cfg(not(target_os = "android"))]
    let tracy = tracy_client::Client::start();
    profiling::scope!("main");

    let event_loop = unsafe { EVENT_LOOP.as_mut().unwrap() };
    let mut window = WindowBuilder::new().build(event_loop).unwrap();

    window.set_title("Xenotech");

    let mut registry = Registry::default();

    registry.create(Graphics::init(&window));
    if let Ok(server) = Server::bind() {
        registry.create(server);
        registry.create(ServerWorld::new());
    }
    registry.create(ClientWorld::new());
    registry.create(Client::connect(&format!("127.0.0.1:{}", net::SERVER_PORT)).unwrap());

    {
        let entity = registry.spawn();
        registry.insert(entity, Translation(SVector::<f32, 3>::new(0.0, 0.0, 20.0)));
        registry.insert(entity, Target(SVector::<f32, 3>::new(0.0, 0.0, 20.0)));
        registry.insert(entity, Look::default());
        registry.insert(entity, Inputs::default());
        registry.insert(entity, Observer { view_distance: 8 });
        registry.insert(entity, Speed(10.4));
        registry.insert(entity, Main);
        registry.insert(entity, ClientTag);
    }

    let mut cursor_captured = false;

    let start_time = time::Instant::now();
    let mut last_delta_time = start_time;
    let mut accum_time = 0.0f32;

    let mut cursor_movement = SVector::<f32, 2>::default();
    let mut observer_input = Input::default();

    let mut curr_block = Block::Machine;

    let mut tick_number = 0usize;

    event_loop.run_return(move |event, _, control_flow| {
        control_flow.set_poll();

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(resolution),
                ..
            } => {
                registry
                    .resource_mut::<Graphics>()
                    .unwrap()
                    .resize(resolution.width, resolution.height);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input: keyboard_input,
                        ..
                    },
                ..
            } => {
                if !cursor_captured {
                    return;
                }

                let Some(keycode) = keyboard_input.virtual_keycode else {
                        return;
                    };

                trait ElementStateExt {
                    fn to_dir(&self, pressed: f32) -> f32;
                }
                impl ElementStateExt for ElementState {
                    fn to_dir(&self, pressed: f32) -> f32 {
                        match &self {
                            ElementState::Pressed => pressed,
                            ElementState::Released => 0.0,
                        }
                    }
                }
                match keycode {
                    VirtualKeyCode::Escape => {
                        cursor_captured = false;
                        window.set_cursor_grab(CursorGrabMode::None).unwrap();
                        window.set_cursor_visible(true);
                    }
                    VirtualKeyCode::Numpad0 => {
                        curr_block = Block::Machine;
                    }

                    VirtualKeyCode::Numpad1 => {
                        curr_block = Block::Wire;
                    }

                    VirtualKeyCode::Numpad2 => {
                        curr_block = Block::Source;
                    }
                    VirtualKeyCode::D => {
                        observer_input.direction.x = keyboard_input.state.to_dir(1.0)
                    }
                    VirtualKeyCode::A => {
                        observer_input.direction.x = keyboard_input.state.to_dir(-1.0)
                    }
                    VirtualKeyCode::W => {
                        observer_input.direction.y = keyboard_input.state.to_dir(1.0)
                    }
                    VirtualKeyCode::S => {
                        observer_input.direction.y = keyboard_input.state.to_dir(-1.0)
                    }
                    VirtualKeyCode::Space => {
                        observer_input.direction.z = keyboard_input.state.to_dir(1.0)
                    }
                    VirtualKeyCode::LShift => {
                        observer_input.direction.z = keyboard_input.state.to_dir(-1.0)
                    }
                    _ => {}
                }
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { button, state, .. },
                ..
            } => {
                if state == ElementState::Pressed
                    && (button == MouseButton::Right || button == MouseButton::Left)
                {
                    cursor_captured = true;
                    window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
                    window.set_cursor_visible(false);
                }
                use band::*;
                match (state, button) {
                    (ElementState::Pressed, MouseButton::Left) => {
                        if let Some((e, _)) = <(Entity, &Main)>::query(&mut registry).next() {
                            registry.insert(e, Change::Place(curr_block));
                        }
                    }
                    (ElementState::Pressed, MouseButton::Right) => {
                        if let Some((e, _)) = <(Entity, &Main)>::query(&mut registry).next() {
                            registry.insert(e, Change::Break(Block::Air));
                        }
                    }
                    _ => {}
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let PhysicalSize { width, height } = window.inner_size();
                if !cursor_captured {
                    return;
                }

                let middle = PhysicalPosition {
                    x: width as f64 / 2.0,
                    y: height as f64 / 2.0,
                };

                if position == middle {
                    return;
                }

                cursor_movement += SVector::<f32, 2>::new(
                    (position.x - middle.x) as f32,
                    (position.y - middle.y) as f32,
                );

                window.set_cursor_position(middle);
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                control_flow.set_exit();
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                let now = time::Instant::now();

                let delta_time = now.duration_since(last_delta_time).as_secs_f32();
                last_delta_time = now;

                const SENSITIVITY: f32 = 2e-3;
                observer_input.gaze = SENSITIVITY * -cursor_movement;
                cursor_movement = SVector::default();

                use band::*;
                if let Some((inputs, _)) = <(&mut Inputs, &Main)>::query(&mut registry).next() {
                    if inputs.state.len() == 0 {
                        inputs
                            .state
                            .push_back((SystemTime::now(), observer_input.clone()));
                    } else if let Some((_, prev_input)) = inputs.state.get(inputs.state.len() - 1) {
                        if *prev_input != observer_input {
                            inputs
                                .state
                                .push_back((SystemTime::now(), observer_input.clone()));
                        }
                    } else {
                        inputs
                            .state
                            .push_back((SystemTime::now(), observer_input.clone()));
                    }
                }

                let graphics = registry.resource_mut::<Graphics>().unwrap();

                if let Some(connection) = registry.destroy::<Connection>() {
                    match connection.check_connection() {
                        Ok(Ok(client)) => registry.create(client),
                        Ok(Err(e)) => panic!("whoopsie daisy, {:?}", e),
                        Err(conn) => registry.create(conn),
                    }
                }

                let server = registry.resource_mut::<Server>();
                let client = registry.resource_mut::<Client>();
                let mut server_world = registry.resource_mut::<ServerWorld>();
                let mut client_world = registry.resource_mut::<ClientWorld>();

                accum_time += delta_time;
                if let Some(world) = &mut server_world && let Some(_) = server {
                    world.delta_tick(&mut registry, delta_time);
                    world.load(&mut registry);
                }
                if let Some(world) = &mut client_world && let Some(_) = client {     
                    world.delta_tick(&mut registry, delta_time);
                }
                while accum_time >= FIXED_TIME {
                    if let Some(world) = &mut server_world && let Some(_) = server {
                        world.fixed_tick(&mut registry, delta_time);
                    }

                    if let Some(world) = &mut client_world && let Some(_) = client {     
                        world.fixed_tick(&mut registry, delta_time);
                        world.display(&mut registry);
                    }
                    accum_time -= FIXED_TIME;
                    tick_number += 1;
                }
                graphics.render(&mut registry);
                //profiling::finish_frame!();
                /*if let Some(world) = client_world {
                    world.cleanup(&mut registry);
                }*/
            }
            _ => (),
        }
    });
}
