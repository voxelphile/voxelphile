use winit::event_loop::EventLoop;

fn main() {
    unsafe { xenotech::EVENT_LOOP = Some(xenotech::EventLoop(EventLoop::new())) };
    xenotech::main();
}
