#version 460
#extension GL_EXT_debug_printf : enable
layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

struct Camera {
    mat4 proj;
    mat4 view;
    mat4 trans;
    uvec2 resolution;
};

layout(binding = 0) buffer GlobalBuffer {
    Camera camera;   
};
uniform layout(binding=1, rgba32ui) readonly uimage2D noise_image; 
uniform layout(binding=2, rgba32f) readonly image1D ssao_kernel_image; 
uniform layout(binding=3, rgba32f) readonly image2D position_image; 
uniform layout(binding=4, rgba32f) readonly image2D normal_image; 
uniform layout(binding=5, rgba32f) writeonly image2D ssao_output_image; 

float float_construct_0_1(uint m)
{
    uint x = m;

    uint ieee_mantissa = 0x007FFFFFu;
    uint ieee_one = 0x3F800000u;

    x &= ieee_mantissa;
    x |= ieee_one;

    float f = uintBitsToFloat(x);

    return f - 1.0;
}

void main()
{
    ivec2 pos_i = ivec2(gl_GlobalInvocationID.xy);
    if(any(greaterThanEqual(pos_i, ivec2(camera.resolution)))) {
        return;
    }
    vec4 frag_pos = imageLoad(position_image, pos_i);
    if(frag_pos.w == 0) {
            imageStore(ssao_output_image, pos_i, vec4(0));
            return;
    }
    frag_pos = camera.view * vec4(frag_pos.xyz, 1.0);
    float occlusion = 0.0;
    {
        vec2 resolution = vec2(camera.resolution);
        vec3 normal = (camera.view * vec4(-imageLoad(normal_image, pos_i).rgb, 0)).xyz;
        uvec2 random_ints = imageLoad(noise_image, pos_i % imageSize(noise_image)).rg;
        vec3 random = normalize(vec3(float_construct_0_1(random_ints.x) * 2 - 1, float_construct_0_1(random_ints.y) * 2 - 1, 0));
        vec3 tangent = normalize(random - normal * dot(random, normal));
        vec3 bitangent = cross(normal, tangent);
        mat3 tbn = mat3(tangent, bitangent, normal);    
        float radius = 0.8;
        for(uint i = 0; i < 8; i++) {
            vec3 sample_pos = tbn * imageLoad(ssao_kernel_image, int(i)).rgb;
            sample_pos = frag_pos.xyz + sample_pos * radius;
            vec4 offset = vec4(sample_pos, 1.0);
            offset = camera.proj * offset;
            offset.xyz /= offset.w;
            offset.xyz = offset.xyz * 0.5 + 0.5;
            ivec2 offset_pos_i = ivec2(vec2(camera.resolution) * offset.xy);
            if(any(lessThan(offset_pos_i, ivec2(0))) || any(greaterThanEqual(offset_pos_i, ivec2(camera.resolution)))) {
                continue;
            }
            vec3 offset_normal = (camera.view * vec4(-imageLoad(normal_image, offset_pos_i).rgb, 0)).xyz;
            if(dot(offset_normal, normal) > 0.99) {
                continue;
            }
            vec3 offset_pos = (camera.view * vec4(imageLoad(position_image, offset_pos_i).xyz, 1.0)).xyz;
            float range_check = smoothstep(0, 1, radius / abs(frag_pos.z - offset_pos.z));
            occlusion += (offset_pos.z <= sample_pos.z - 0.005 ? 0.0 : 1.0) * range_check;
        }
    }
    occlusion = 1.0 - (occlusion / 8);
    
    imageStore(ssao_output_image, pos_i, vec4(occlusion));
}