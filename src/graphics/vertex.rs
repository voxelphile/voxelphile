use nalgebra::SVector;

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct BlockVertex {
    pub data: SVector<u32, 4>,
}

impl BlockVertex {
    pub fn new(
        position: SVector<usize, 3>,
        uv: SVector<f32, 2>,
        mut color: SVector<f32, 4>,
    ) -> BlockVertex {
        let mut data = SVector::<u32, 4>::default();

        data.x = ((position.x as u32 & 0xFF) << 16)
            | ((position.y as u32 & 0xFF) << 8)
            | ((position.z as u32) & 0xFF);
        color *= 255.0;
        data.z = ((color.x as u32 & 0xFF) << 24)
            | ((color.y as u32 & 0xFF) << 16)
            | ((color.z as u32 & 0xFF) << 8)
            | ((color.w as u32) & 0xFF);

        Self { data }
    }
}
