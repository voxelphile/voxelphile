use nalgebra::SVector;
use serde_derive::{Serialize, Deserialize};

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Input {
    pub gaze: SVector<f32, 2>,
    pub direction: SVector<f32, 3>,
}
