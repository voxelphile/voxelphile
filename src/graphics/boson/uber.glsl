#version 460
#define shader_type_vertex 1
#define shader_type_fragment 2

struct Vertex {
    uvec4 data;
};
struct DrawIndirectCommand {
    uint vertex_count;
    uint instance_count;
    uint first_vertex;
    uint first_instance;
};
struct Camera {
    mat4 proj;
    mat4 view;
};
struct IndirectData {
    DrawIndirectCommand cmd;
    vec4 position;
};

layout(binding = 0) buffer VertexBuffer {
    Vertex vertices[];
};
layout(binding = 1) buffer IndexBuffer {
    uint indices[];
};
layout(binding = 2) buffer IndirectBuffer {
    IndirectData indirect[];
};
layout(binding = 3) buffer GlobalBuffer {
    Camera camera;   
};
layout(binding=4, rgba32f) readonly uniform image3D atlas; 

#if shader_type == shader_type_vertex

layout(location = 0) out vec4 position;
layout(location = 1) out vec2 uv;
layout(location = 2) out flat uint mapping;
layout(location = 3) out vec4 tint;

void unpack(uvec4 data, out vec4 position, out vec2 uv, out uint mapping, out vec4 tint) {
    position.x = float((data.x >> 16) & 0xFF);
    position.y = float((data.x >> 8) & 0xFF);
    position.z = float((data.x) & 0xFF);
    position.w = 1.0;

    uv = vec2(float((data.y >> 16) & 0xFFFF) / float(0xFFFF), float(data.y & 0xFFFF) / float(0xFFFF));

    mapping = data.z;

    tint.x = float((data.w >> 24) & 0xFF) / 255.0;
    tint.y = float((data.w >> 16) & 0xFF) / 255.0;
    tint.z = float((data.w >> 8) & 0xFF) / 255.0;
    tint.w = float((data.w) & 0xFF) / 255.0;
}

void main() {
    Vertex vertex = vertices[indices[gl_VertexIndex]];

    unpack(vertex.data, position, uv, mapping, tint);

    position = vec4(indirect[gl_DrawID].position.xyz + position.xyz, 1);

	gl_Position = camera.proj * camera.view * position;
}

#elif shader_type == shader_type_fragment

layout(location = 0) in vec4 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in flat uint mapping;
layout(location = 3) in vec4 tint;

layout(location = 0) out vec4 result;

#define TEX_SIZE 128
#define ATLAS_AXIS 16

void main() { 
    uvec2 tex_pos = uvec2(mapping % ATLAS_AXIS, mapping / ATLAS_AXIS);
    uvec2 tex_coord = uvec2((vec2(tex_pos) + uv) * TEX_SIZE);
    vec4 albedo = imageLoad(atlas, ivec3(tex_coord, 0));
    result = tint * albedo;
}

#endif