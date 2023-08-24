use winit::window::Window;

#[cfg(feature = "boson")]
mod boson;
#[cfg(feature = "boson")]
pub type Graphics = boson::Boson;

pub trait GraphicsInterface {
    fn init(window: &Window) -> Self;
    fn render(&mut self);
}