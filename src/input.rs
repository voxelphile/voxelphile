use nalgebra::SVector;

#[derive(Default)]
pub struct Input {
    pub gaze: SVector<f32, 2>,
    pub direction: SVector<f32, 3>,
}