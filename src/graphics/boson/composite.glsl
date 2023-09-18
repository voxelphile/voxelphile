#version 460
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
uniform layout(binding=1, rgba32f) readonly image2D ssao_image; 
uniform layout(binding=2, rgba32f) readonly image2D color_image; 
uniform layout(binding=3, rgba32f) image2D composite_image; 

void main()
{
    ivec2 pos_i = ivec2(gl_GlobalInvocationID.xy);
    if(any(greaterThanEqual(pos_i, ivec2(camera.resolution)))) {
        return;
    }
    vec4 res = vec4(imageLoad(ssao_image, pos_i).r * imageLoad(color_image, pos_i).rgb, 1.0);
    imageStore(composite_image, pos_i, res);
}