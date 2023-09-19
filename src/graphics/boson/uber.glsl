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
    mat4 trans;
    uvec2 resolution;
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
layout(location = 4) out flat ivec3 normal;

void unpack(uvec4 data, out vec4 position, out uint lod, out vec2 uv, out uint dir, out uint mapping, out vec4 tint) {
    position.x = float((data.x >> 16) & 0xFF);
    position.y = float((data.x >> 8) & 0xFF);
    position.z = float((data.x) & 0xFF);
    position.w = 1.0;
    lod = (data.x >> 24) & 0xFF;

    uv = vec2(float((data.y >> 16) & 0xFFFF) / float(0xFFFF), float(data.y & 0xFFFF) / float(0xFFFF));

    dir = (data.z >> 24) & 0xFF;
    mapping = data.z & 0xFFFFFF;

    tint.x = float((data.w >> 24) & 0xFF) / 255.0;
    tint.y = float((data.w >> 16) & 0xFF) / 255.0;
    tint.z = float((data.w >> 8) & 0xFF) / 255.0;
    tint.w = float((data.w) & 0xFF) / 255.0;
}

void main() {
    Vertex vertex = vertices[indices[gl_VertexIndex]];

    uint dir;
    uint lod;
    unpack(vertex.data, position, lod, uv, dir, mapping, tint);
    uv *= float(lod);
    if(dir == 0) {
        normal = ivec3(1,0,0);
    }
    if(dir == 1) {
        normal = ivec3(-1,0,0);
    }
    if(dir == 2) {
        normal = ivec3(0,1,0);
    }
    if(dir == 3) {
        normal = ivec3(0,-1,0);
    }
    if(dir == 4) {
        normal = ivec3(0,0,1);
    }
    if(dir == 5) {
        normal = ivec3(0,0,-1);
    }
    position = vec4(indirect[gl_DrawID].position.xyz + position.xyz, 1);

	gl_Position = camera.proj * camera.view * position;
}

#elif shader_type == shader_type_fragment

layout(location = 0) in vec4 position;
layout(location = 1) in vec2 uv;
layout(location = 2) in flat uint mapping;
layout(location = 3) in vec4 tint;
layout(location = 4) in flat ivec3 normal;

layout(location = 0) out vec4 col;
layout(location = 1) out vec4 pos;
layout(location = 2) out vec4 norm;

#define TEX_SIZE 16
#define ATLAS_AXIS 16

void main() { 
    vec2 tex_pos_id = vec2(mapping % ATLAS_AXIS, mapping / ATLAS_AXIS);

    vec2 uvs = tex_pos_id + mod(uv, 1);

    vec2 dx = dFdx(uvs) * 0.25;
    vec2 dy = dFdy(uvs) * 0.25;

    float epsilon = 0.01;

    vec3 albedo = vec3(0); 
    albedo += imageLoad(atlas, ivec3(clamp(uvs.xy + dx + dy, tex_pos_id + epsilon, tex_pos_id + 1 - epsilon) * TEX_SIZE, 0)).rgb;
    albedo += imageLoad(atlas, ivec3(clamp(uvs.xy - dx + dy, tex_pos_id + epsilon, tex_pos_id + 1 - epsilon) * TEX_SIZE, 0)).rgb;
    albedo += imageLoad(atlas, ivec3(clamp(uvs.xy + dx - dy, tex_pos_id + epsilon, tex_pos_id + 1 - epsilon) * TEX_SIZE, 0)).rgb;
    albedo += imageLoad(atlas, ivec3(clamp(uvs.xy - dx - dy, tex_pos_id + epsilon, tex_pos_id + 1 - epsilon) * TEX_SIZE, 0)).rgb;
    albedo *= 0.25;
    col = tint * vec4(albedo, 1);
    pos = position;
    norm = vec4(vec3(normal), 0);
}

#endif