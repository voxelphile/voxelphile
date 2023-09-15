use std::collections::HashMap;

use crate::graphics::boson::StagingBuffer;
use crate::world::structure::Block;
use boson::prelude::{Device, Format, Image, ImageInfo, ImageUsage};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub struct Atlas {
    mapping: HashMap<Block, u32>,
    image: Image,
}

#[derive(EnumIter)]
enum ImageType {
    Albedo,
    Heightmap,
    Normal,
}

impl ImageType {
    fn compatible(&self, block: &Block) -> bool {
        use ImageType::*;
        block.texture_name().is_some()
            && match self {
                Heightmap => block.parallax(),
                Normal => block.normal(),
                _ => true,
            }
    }
}

const TEX_SIZE: usize = 128;
const ATLAS_AXIS: usize = 16;

fn load_image_data<S: AsRef<str>>(str: S, ty: ImageType) -> Vec<f32> {
    use image::io::Reader as ImageReader;
    use std::io::Cursor;
    use ImageType::*;

    let mut name = str.as_ref().to_owned();
    name = String::from("./assets/") + &name;
    name = name
        + match ty {
            Heightmap => "_s",
            Normal => "_n",
            _ => "",
        };
    name = name + ".png";
    dbg!(&name);
    let img = ImageReader::open(&name).unwrap().decode().unwrap();

    let rgba32f = img.into_rgba32f();

    rgba32f.pixels().flat_map(|p| p.0).collect::<Vec<_>>()
}

impl Atlas {
    pub fn load(device: &Device, staging: &mut StagingBuffer) -> Self {
        let mut cursor = 0;

        let depth = ImageType::iter().count();

        let image = device
            .create_image(ImageInfo {
                extent: boson::prelude::ImageExtent::ThreeDim(
                    ATLAS_AXIS * TEX_SIZE,
                    ATLAS_AXIS * TEX_SIZE,
                    depth,
                ),
                usage: ImageUsage::COLOR | ImageUsage::TRANSFER_DST,
                format: Format::Rgba32Sfloat,
                debug_name: "atlas",
            })
            .unwrap();

        let mut mapping = HashMap::<Block, u32>::new();

        for block in Block::iter() {
            let Some(name) = block.texture_name() else {
                continue;
            };
            mapping.insert(block, cursor);

            let x = cursor as usize % ATLAS_AXIS;
            let y = cursor as usize / ATLAS_AXIS;

            for (i, ty) in ImageType::iter().enumerate() {
                if !ty.compatible(&block) {
                    continue;
                }

                let pixel_data = load_image_data(&name, ty);

                staging.upload_image(
                    image,
                    (x * TEX_SIZE, y * TEX_SIZE, i),
                    (TEX_SIZE, TEX_SIZE, 1),
                    &pixel_data,
                );
            }

            cursor += 1;
        }

        Atlas { mapping, image }
    }

    pub fn image(&self) -> Image {
        self.image
    }

    pub fn block_mapping(&self, block: &Block) -> Option<u32> {
        self.mapping.get(block).copied()
    }
}
