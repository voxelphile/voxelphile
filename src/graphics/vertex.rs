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
        direction: u8,
        mapping: u32,
        mut tint: SVector<f32, 4>,
    ) -> BlockVertex {
        let mut data = SVector::<u32, 4>::default();

        data.x = ((position.x as u32 & 0xFF) << 16)
            | ((position.y as u32 & 0xFF) << 8)
            | ((position.z as u32) & 0xFF);
        data.y = (((uv.x * 0xFFFF as f32) as u32) << 16) | ((uv.y * 0xFFFF as f32) as u32);
        data.z = ((direction as u32) << 24) | ((mapping as u32) & 0xFFFFFF);
        tint *= 255.0;
        data.w = ((tint.x as u32 & 0xFF) << 24)
            | ((tint.y as u32 & 0xFF) << 16)
            | ((tint.z as u32 & 0xFF) << 8)
            | ((tint.w as u32) & 0xFF);

        Self { data }
    }
}
