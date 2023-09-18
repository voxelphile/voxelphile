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
    {
        let source = fs::read_to_string("src/graphics/boson/postfx.glsl").unwrap();
        let postfx = {
            let mut options = shaderc::CompileOptions::new().unwrap();
            options.add_macro_definition("shader_type", Some("shader_type_compute"));
            options.set_generate_debug_info();
            compiler
                .compile_into_spirv(
                    &source,
                    shaderc::ShaderKind::Compute,
                    "postfx.glsl",
                    "main",
                    Some(&options),
                )
                .unwrap()
        };
        fs::write("assets/postfx.comp.spirv", postfx.as_binary_u8()).unwrap();
    }
    {
        let source = fs::read_to_string("src/graphics/boson/blur.glsl").unwrap();
        let blur = {
            let mut options = shaderc::CompileOptions::new().unwrap();
            options.add_macro_definition("shader_type", Some("shader_type_compute"));
            options.set_generate_debug_info();
            compiler
                .compile_into_spirv(
                    &source,
                    shaderc::ShaderKind::Compute,
                    "blur.glsl",
                    "main",
                    Some(&options),
                )
                .unwrap()
        };
        fs::write("assets/blur.comp.spirv", blur.as_binary_u8()).unwrap();
    }
    {
        let source = fs::read_to_string("src/graphics/boson/composite.glsl").unwrap();
        let composite = {
            let mut options = shaderc::CompileOptions::new().unwrap();
            options.add_macro_definition("shader_type", Some("shader_type_compute"));
            options.set_generate_debug_info();
            compiler
                .compile_into_spirv(
                    &source,
                    shaderc::ShaderKind::Compute,
                    "composite.glsl",
                    "main",
                    Some(&options),
                )
                .unwrap()
        };
        fs::write("assets/composite.comp.spirv", composite.as_binary_u8()).unwrap();
    }
}
