#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub type Window = windows::Window;

pub trait WindowInterface {
    fn open() -> Self;
    fn poll(&self);
    fn close(self);
}