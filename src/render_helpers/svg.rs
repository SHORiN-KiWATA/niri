use resvg::tiny_skia::{Pixmap, Transform};
use usvg::{Options, Tree};

/// Renders an SVG cursor at the requested target size.
///
/// Returns `(width, height, pixels_rgba, xhot, yhot)`.
pub fn render_cursor(
    svg_data: &str,
    target_size: u32,
) -> Option<(u32, u32, Vec<u8>, u32, u32)> {
    let opts = Options::default();
    let tree = Tree::from_str(svg_data, &opts).ok()?;

    let svg_size = tree.size();
    let scale_w = target_size as f32 / svg_size.width();
    let scale_h = target_size as f32 / svg_size.height();
    let scale = scale_w.min(scale_h);

    let width = (svg_size.width() * scale).ceil() as u32;
    let height = (svg_size.height() * scale).ceil() as u32;

    let mut pixmap = Pixmap::new(width, height)?;

    let transform = Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let pixels_rgba = pixmap.data().to_vec();

    let (xhot, yhot) = parse_hotspot(svg_data);
    let xhot = (xhot as f32 * scale).round() as u32;
    let yhot = (yhot as f32 * scale).round() as u32;

    debug!(
        "rendered SVG cursor: {width}x{height} (target {target_size}), hotspot ({xhot}, {yhot})"
    );

    Some((width, height, pixels_rgba, xhot, yhot))
}

fn parse_hotspot(svg_data: &str) -> (u32, u32) {
    for line in svg_data.lines() {
        if let Some(pos) = line.find("Hotspot:") {
            let rest = &line[pos + 8..].trim();
            if let Some((x_str, y_str)) = rest.split_once(',') {
                let x: u32 = x_str.trim().parse().ok().unwrap_or(0);
                let y: u32 = y_str.trim().parse().ok().unwrap_or(0);
                return (x.min(128), y.min(128));
            }
        }
    }
    (1, 1)
}
