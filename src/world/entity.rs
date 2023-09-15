use nalgebra::SVector;

use crate::input::Input;

pub struct Hitbox {
    offset: SVector<f32, 3>,
    size: SVector<f32, 3>,
}

pub enum Entity {
    Player {
        translation: SVector<f32, 3>,
        look: SVector<f32, 2>,
        input: Input,
        speed: f32,
    },
}

impl Entity {
    pub fn hitbox(&self) -> Hitbox {
        use Entity::*;
        match self {
            Player { .. } => Hitbox {
                offset: SVector::<f32, 3>::new(0.0, 0.0, 0.7),
                size: SVector::<f32, 3>::new(0.4, 0.4, 1.8),
            },
        }
    }
    
}
