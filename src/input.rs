use std::{time::SystemTime, collections::VecDeque};

use nalgebra::SVector;
use serde_derive::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Input {
    pub gaze: SVector<f32, 2>,
    pub direction: SVector<f32, 3>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Inputs {
    pub state: VecDeque<(SystemTime, Input)>,
}
