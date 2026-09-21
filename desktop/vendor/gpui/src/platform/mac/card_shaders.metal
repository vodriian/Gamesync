#include <metal_stdlib>
using namespace metal;
struct CardUniforms { float4 rect; float4 viewport_pose; float4 shape; float4 clip; float4 texture_region; };
struct CardVertex { float4 position [[position]]; float2 uv; };
vertex CardVertex card_vertex(uint vertex_id [[vertex_id]], constant CardUniforms &u [[buffer(0)]]) {
    const float2 vertices[] = {float2(0,0),float2(1,0),float2(0,1),float2(0,1),float2(1,0),float2(1,1)};
    float2 uv = vertices[vertex_id];
    float2 dimensions = u.rect.zw;
    float pitch = u.viewport_pose.z;
    float yaw = u.viewport_pose.w - u.shape.y * M_PI_F;
    float2 local = (uv - .5) * dimensions;
    float z = 0.;
    if (u.shape.w == 2.) { local *= (dimensions + 160.) / dimensions; uv = local / dimensions + .5; }
    if (u.shape.w == 1.) {
        // A thin physical edge remains visible when the face is edge-on.
        local.x = (sin(yaw) >= 0. ? 1. : -1.) * (dimensions.x / 2. - .8);
        local.y *= (dimensions.y - 2. * u.shape.x) / dimensions.y;
        z = (uv.x - .5) * 3.;
    }
    float3 p = float3(local.x * cos(yaw) + z * sin(yaw), local.y, -local.x * sin(yaw) + z * cos(yaw));
    p = float3(p.x, p.y * cos(pitch) - p.z * sin(pitch), p.y * sin(pitch) + p.z * cos(pitch));
    float distance = max(dimensions.x, dimensions.y) * 3.;
    float w = 1. - p.z / distance;
    float2 center = u.rect.xy + dimensions * .5;
    if (u.shape.w == 2.) { center += float2(-sin(yaw) * 9., 18. + abs(sin(pitch)) * 9.); }
    float2 screen = center + p.xy / w;
    float2 ndc = screen / u.viewport_pose.xy * 2. - 1.;
    return {float4(ndc.x * w, -ndc.y * w, 0., w), uv};
}
float card_sdf(float2 uv, float2 dimensions, float radius) {
    float2 q = abs((uv - .5) * dimensions) - dimensions * .5 + radius;
    return length(max(q, 0.)) + min(max(q.x,q.y),0.) - radius;
}
fragment float4 card_fragment(CardVertex in [[stage_in]], constant CardUniforms &u [[buffer(0)]], texture2d<float> face [[texture(0)]]) {
    if (any(in.position.xy < u.clip.xy) || any(in.position.xy > u.clip.xy + u.clip.zw)) { discard_fragment(); }
    float yaw = u.viewport_pose.w - u.shape.y * M_PI_F;
    if (cos(yaw) * cos(u.viewport_pose.z) < 0.) { discard_fragment(); }
    float d = card_sdf(in.uv, u.rect.zw, u.shape.x);
    if (u.shape.w == 2.) { float a = .10 * exp(-max(d,0.) / 28.) * (1. - smoothstep(0.,80.,d)); return float4(float3(.025)*a,a); }
    if (u.shape.w == 1.) { float a = .9; return float4(float3(.38,.40,.43)*a,a); }
    constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);
    float4 color = face.sample(s, in.uv * u.texture_region.xy);
    if (u.texture_region.z > 0. && in.uv.y < u.texture_region.z) {
        // Blur only the strip behind toolbar controls, reusing the captured GPU
        // viewport. The remaining library stays sharp and keeps native hitboxes.
        float4 blurred = float4(0.); float weight = 0.;
        float2 texel = 1. / float2(face.get_width(), face.get_height());
        for (int y = -3; y <= 3; ++y) {
            for (int x = -3; x <= 3; ++x) {
                float w = exp(-float(x*x+y*y) / 5.);
                float2 uv = clamp(in.uv * u.texture_region.xy + float2(x,y) * 8. * texel,
                    float2(0.), u.texture_region.xy - texel);
                blurred += face.sample(s, uv) * w; weight += w;
            }
        }
        color = blurred / weight;
    }
    float aa = max(fwidth(d), .7);
    color *= 1. - smoothstep(-aa * .5, aa * .5, d);
    if (u.shape.z > .5 && u.shape.y < .5) {
        float light = .97 + .03 * cos(yaw);
        float band = exp(-pow((in.uv.x + in.uv.y * .34 - .55 - sin(yaw) * .5 - sin(u.viewport_pose.z) * .2) * 3.5, 2.));
        color.rgb = color.rgb * light + float3(.045,.047,.05) * band * color.a;
    }
    return color;
}
