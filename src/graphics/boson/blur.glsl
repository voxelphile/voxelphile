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
uniform layout(binding=1, rgba32f) readonly image2D normal_image; 
uniform layout(binding=2, rgba32f) image2D blur_image; 

layout(push_constant) uniform PushConstant
{
	uint dir;
} push;

void main()
{
    ivec2 pos_i = ivec2(gl_GlobalInvocationID.xy);
    if(any(greaterThanEqual(pos_i, ivec2(camera.resolution)))) {
        return;
    }
    vec4 _res_info = vec4(0);
    vec4 _input_normal = imageLoad(normal_image, pos_i);
    uint samples = 0;
    int dist = 4;
    for (int i = -dist; i <= dist; i++) {
        ivec2 off = ivec2(0);
        if (push.dir == 0) {
            off.x = 1;
        }
        if (push.dir == 1) {
            off.y = 1;
        }
        ivec2 off_i = pos_i + i * off;
        if(any(lessThan(off_i, ivec2(0))) || any(greaterThanEqual(off_i, ivec2(camera.resolution)))) {
            continue;
        }
        vec4 _rel_normal = imageLoad(normal_image, off_i);
        if(dot(_input_normal, _rel_normal) < 0.9) {
            continue;
        }
        samples++;
        _res_info += imageLoad(blur_image, off_i);
    }
    _res_info /= float(samples);
    imageStore(blur_image, pos_i, _res_info);
}