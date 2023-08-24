mod window;
mod graphics;
use window::*;
use graphics::*;


fn main() {
    println!("Hello, xenotech!");

    let window = Window::open();
    let graphics = Graphics::init();

    loop {
        window.poll();
    }
}
