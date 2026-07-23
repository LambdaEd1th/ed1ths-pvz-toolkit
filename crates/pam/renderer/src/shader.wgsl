struct ViewUniform {
    viewport: vec2<f32>,
    _padding: vec2<f32>,
    corner_radii: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) pixel: vec2<f32>,
};

@group(0) @binding(0) var<uniform> view: ViewUniform;
@group(1) @binding(0) var image_texture: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

fn rounded_corner_coverage(pixel: vec2<f32>) -> f32 {
    let left = pixel.x < view.viewport.x * 0.5;
    let top = pixel.y < view.viewport.y * 0.5;
    let radius = select(
        select(view.corner_radii.w, view.corner_radii.z, !left),
        select(view.corner_radii.x, view.corner_radii.y, !left),
        top,
    );
    if radius <= 0.0 {
        return 1.0;
    }

    let center = vec2<f32>(
        select(view.viewport.x - radius, radius, left),
        select(view.viewport.y - radius, radius, top),
    );
    let in_corner = select(pixel.x > view.viewport.x - radius, pixel.x < radius, left)
        && select(pixel.y > view.viewport.y - radius, pixel.y < radius, top);
    if !in_corner {
        return 1.0;
    }

    let distance = length(pixel - center) - radius;
    return 1.0 - smoothstep(-0.75, 0.75, distance);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) matrix: vec4<f32>,
    @location(1) translation_size: vec4<f32>,
    @location(2) instance_uv: vec4<f32>,
    @location(3) instance_color: vec4<f32>,
) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let local = corner * translation_size.zw;
    let pixel = vec2<f32>(
        matrix.x * local.x + matrix.z * local.y + translation_size.x,
        matrix.y * local.x + matrix.w * local.y + translation_size.y,
    );

    var output: VertexOutput;
    output.position = vec4<f32>(
        pixel.x / view.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / view.viewport.y * 2.0,
        0.0,
        1.0,
    );
    output.uv = mix(instance_uv.xy, instance_uv.zw, corner);
    output.color = instance_color;
    output.pixel = pixel;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = rounded_corner_coverage(input.pixel);
    let sampled = textureSample(image_texture, image_sampler, input.uv);
    if input.color.a < 0.0 {
        let tile = (u32(floor(input.pixel.x / 10.0)) + u32(floor(input.pixel.y / 10.0))) & 1u;
        let light_background = input.color.r > 0.5;
        let offset = select(0.006, -0.055, light_background);
        let alternate = select(0.0, offset, tile == 1u);
        return vec4<f32>(input.color.rgb + vec3<f32>(alternate), 1.0) * coverage;
    }
    let alpha = sampled.a * input.color.a;
    return vec4<f32>(sampled.rgb * input.color.rgb * alpha, alpha) * coverage;
}
