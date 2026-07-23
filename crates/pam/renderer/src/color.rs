pub(crate) fn srgb8_to_linear(value: u8) -> f32 {
    let encoded = f32::from(value) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn linear_rgb_from_srgb8(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(srgb8_to_linear)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn wgpu_color_from_srgb8(rgb: [u8; 3]) -> wgpu::Color {
    let [r, g, b] = linear_rgb_from_srgb8(rgb);
    wgpu::Color {
        r: f64::from(r),
        g: f64::from(g),
        b: f64::from(b),
        a: 1.0,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
#[cfg(any(test, all(target_arch = "wasm32", feature = "web-host")))]
pub(crate) fn srgb_view_format(format: wgpu::TextureFormat) -> Option<wgpu::TextureFormat> {
    let format = format.add_srgb_suffix();
    format.is_srgb().then_some(format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_srgb_bytes_to_linear_channels() {
        assert_eq!(srgb8_to_linear(0), 0.0);
        assert_eq!(srgb8_to_linear(255), 1.0);
        assert!((srgb8_to_linear(128) - 0.215_860_53).abs() < 0.000_001);
        assert!(srgb8_to_linear(16) < 16.0 / 255.0);
    }

    #[test]
    fn selects_srgb_views_for_canvas_formats() {
        assert_eq!(
            srgb_view_format(wgpu::TextureFormat::Bgra8Unorm),
            Some(wgpu::TextureFormat::Bgra8UnormSrgb)
        );
        assert_eq!(
            srgb_view_format(wgpu::TextureFormat::Rgba8UnormSrgb),
            Some(wgpu::TextureFormat::Rgba8UnormSrgb)
        );
        assert_eq!(srgb_view_format(wgpu::TextureFormat::R16Float), None);
    }
}
