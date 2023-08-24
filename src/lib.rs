mod graphics;
use graphics::*;
use std::ops;
use winit::{event::{Event, WindowEvent}, window::WindowBuilder, platform::run_return::EventLoopExtRunReturn};


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

    event_loop.run_return(move |event, _, control_flow| {
        control_flow.set_poll();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("The close button was pressed; stopping");
                control_flow.set_exit();
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
               graphics.render();
            }
            _ => (),
        }}
    );
}
