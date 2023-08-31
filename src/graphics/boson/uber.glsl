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

#if shader_type == shader_type_vertex

layout(location = 0) out vec4 position;
layout(location = 1) out vec2 uv;
layout(location = 2) out vec4 color;

void unpack(uvec4 data, out vec4 position, out vec2 uv, out vec4 color) {
    position.x = float((data.x >> 16) & 0xFF);
    position.y = float((data.x >> 8) & 0xFF);
    position.z = float((data.x) & 0xFF);
    position.w = 1.0;

    uv = vec2(0);

    color.x = float((data.z >> 24) & 0xFF) / 255.0;
    color.y = float((data.z >> 16) & 0xFF) / 255.0;
    color.z = float((data.z >> 8) & 0xFF) / 255.0;
    color.w = float((data.z) & 0xFF) / 255.0;
}

void main() {
    Vertex vertex = vertices[indices[gl_VertexIndex]];

    unpack(vertex.data, position, uv, color);

    position = vec4(indirect[gl_DrawID].position.xyz, 0) + position;

	gl_Position = camera.proj * camera.view * position;
}

#elif shader_type == shader_type_fragment

layout(location = 0) in vec4 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in vec4 color;

layout(location = 0) out vec4 result;

void main() {   
    result = color;
}

#endif