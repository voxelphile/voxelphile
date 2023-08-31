use std::fs;

pub fn main() {
    let compiler = shaderc::Compiler::new().unwrap();
    {
        let source = fs::read_to_string("src/graphics/boson/uber.glsl").unwrap();
        let vertex = {
            let mut options = shaderc::CompileOptions::new().unwrap();
            options.add_macro_definition("shader_type", Some("shader_type_vertex"));
            compiler
                .compile_into_spirv(
                    &source,
                    shaderc::ShaderKind::Vertex,
                    "uber.glsl",
                    "main",
                    Some(&options),
                )
                .unwrap()
        };

        let fragment = {
            let mut options = shaderc::CompileOptions::new().unwrap();
            options.add_macro_definition("shader_type", Some("shader_type_fragment"));
            compiler
                .compile_into_spirv(
                    &source,
                    shaderc::ShaderKind::Fragment,
                    "uber.glsl",
                    "main",
                    Some(&options),
                )
                .unwrap()
        };

        fs::write("assets/uber.vert.spirv", vertex.as_binary_u8()).unwrap();
        fs::write("assets/uber.frag.spirv", fragment.as_binary_u8()).unwrap();
    }
}
