mod graphics;
mod world;
use graphics::{vertex::BlockVertex, *};
use nalgebra::{SVector, Unit, UnitQuaternion};
use std::{f32::consts::PI, ops, time};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, MouseButton, VirtualKeyCode, WindowEvent},
    platform::run_return::EventLoopExtRunReturn,
    window::{CursorGrabMode, WindowBuilder},
};
use world::{
    structure::{gen_block_mesh, gen_chunk, CHUNK_AXIS},
    World,
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

pub fn main() {
    let event_loop = unsafe { EVENT_LOOP.as_mut().unwrap() };
    let mut window = WindowBuilder::new().build(event_loop).unwrap();

    window.set_title("Xenotech");

    let mut graphics = Graphics::init(&window);

    let mut world = World::new(48);

    let mut cursor_captured = false;
    let mut cursor_movement = SVector::<f32, 2>::new(0.0, 0.0);
    let mut direction = SVector::<f32, 3>::new(0.0, 0.0, 0.0);
    let mut look = SVector::<f32, 2>::new(0.0, 0.0);
    let mut translation = SVector::<f32, 3>::new(0.0, 0.0, 0.0);
    

    let start_time = time::Instant::now();
    let mut last_delta_time = start_time;

    event_loop.run_return(move |event, _, control_flow| {
        control_flow.set_poll();

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(resolution),
                ..
            } => {
                graphics.resize(resolution.width, resolution.height);
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
                    VirtualKeyCode::D => direction.x = keyboard_input.state.to_dir(1.0),
                    VirtualKeyCode::A => direction.x = keyboard_input.state.to_dir(-1.0),
                    VirtualKeyCode::W => direction.y = keyboard_input.state.to_dir(1.0),
                    VirtualKeyCode::S => direction.y = keyboard_input.state.to_dir(-1.0),
                    VirtualKeyCode::Space => direction.z = keyboard_input.state.to_dir(1.0),
                    VirtualKeyCode::LShift => direction.z = keyboard_input.state.to_dir(-1.0),
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

                let sensitivity = 2e-3;
                look.x -= sensitivity * cursor_movement.x;
                look.y -= sensitivity * cursor_movement.y;
                look.y = look.y.clamp(0.0, PI);

                cursor_movement = Default::default();

                translation += 0.25 * delta_time
                    * (UnitQuaternion::from_axis_angle(
                        &Unit::new_normalize(SVector::<f32, 3>::new(0.0, 0.0, 1.0)),
                        look.x,
                    )
                    .to_rotation_matrix()
                        * direction);

               
                world.load(&mut graphics, translation);

                graphics.render(look, translation);
            }
            _ => (),
        }
    });
}
