use crate::core::svg;
use crate::core::{Color, Size};
use crate::image::atlas::{self, Atlas};

use resvg::tiny_skia;
use resvg::usvg;
use rustc_hash::{FxHashMap, FxHashSet};
use std::fs;
use std::panic;
use std::sync::Arc;

/// Entry in cache corresponding to an svg handle
pub enum Svg {
    /// Parsed svg
    Loaded(usvg::Tree),
    /// Svg not found or failed to parse
    NotFound,
}

impl Svg {
    /// Viewport width and height
    pub fn viewport_dimensions(&self) -> Size<u32> {
        match self {
            Svg::Loaded(tree) => {
                let size = tree.size();

                Size::new(size.width() as u32, size.height() as u32)
            }
            Svg::NotFound => Size::new(1, 1),
        }
    }
}

/// Caches svg vector and raster data
#[derive(Debug, Default)]
pub struct Cache {
    svgs: FxHashMap<u64, Svg>,
    rasterized: FxHashMap<(u64, u32, u32, ColorFilter), atlas::Entry>,
    svg_hits: FxHashSet<u64>,
    rasterized_hits: FxHashSet<(u64, u32, u32, ColorFilter)>,
    should_trim: bool,
    fontdb: Option<Arc<usvg::fontdb::Database>>,
}

type ColorFilter = Option<[u8; 4]>;

impl Cache {
    /// Load svg
    pub fn load(&mut self, handle: &svg::Handle) -> &Svg {
        if self.svgs.contains_key(&handle.id()) {
            return self.svgs.get(&handle.id()).unwrap();
        }

        // TODO: Reuse `cosmic-text` font database
        if self.fontdb.is_none() {
            let mut fontdb = usvg::fontdb::Database::new();
            fontdb.load_system_fonts();

            self.fontdb = Some(Arc::new(fontdb));
        }

        let options = usvg::Options {
            fontdb: self
                .fontdb
                .as_ref()
                .expect("fontdb must be initialized")
                .clone(),
            ..usvg::Options::default()
        };

        let svg = match handle.data() {
            svg::Data::Path(path) => fs::read_to_string(path)
                .ok()
                .and_then(|contents| usvg::Tree::from_str(&contents, &options).ok())
                .map(Svg::Loaded)
                .unwrap_or(Svg::NotFound),
            svg::Data::Bytes(bytes) => match usvg::Tree::from_data(bytes, &options) {
                Ok(tree) => Svg::Loaded(tree),
                Err(_) => Svg::NotFound,
            },
        };

        self.should_trim = true;

        let _ = self.svgs.insert(handle.id(), svg);
        self.svgs.get(&handle.id()).unwrap()
    }

    /// Load svg and upload raster data
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        handle: &svg::Handle,
        color: Option<Color>,
        size: Size<u32>,
        atlas: &mut Atlas,
    ) -> Option<&atlas::Entry> {
        let id = handle.id();

        let color = color.map(Color::into_rgba8);
        let key = (id, size.width, size.height, color);

        // TODO: Optimize!
        // We currently rerasterize the SVG when its size changes. This is slow
        // as heck. A GPU rasterizer like `pathfinder` may perform better.
        // It would be cool to be able to smooth resize the `svg` example.
        if self.rasterized.contains_key(&key) {
            let _ = self.svg_hits.insert(id);
            let _ = self.rasterized_hits.insert(key);

            return self.rasterized.get(&key);
        }

        match self.load(handle) {
            Svg::Loaded(tree) => {
                // TODO: Optimize!
                // We currently rerasterize the SVG when its size changes. This is slow
                // as heck. A GPU rasterizer like `pathfinder` may perform better.
                // It would be cool to be able to smooth resize the `svg` example.
                let mut img = tiny_skia::Pixmap::new(size.width, size.height)?;

                // The widget already resolved aspect ratio and physical bounds.
                // Rasterize straight into those exact pixel dimensions. Rounding
                // a second aspect-fit size here distorted/cropped narrow glyphs.
                let transform = raster_transform(tree, size);

                // SVG rendering can panic on malformed or complex vectors.
                // We catch panics to prevent crashes and continue gracefully.
                let render = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    resvg::render(tree, transform, &mut img.as_mut());
                }));

                if let Err(error) = render {
                    log::warn!("SVG rendering for {handle:?} panicked: {error:?}");
                }

                // tiny-skia returns premultiplied RGBA; the shared image
                // shader expects straight RGBA and premultiplies once itself.
                // Uploading the original data darkened/thinned antialiased edges.
                let mut rgba = straight_rgba(img);

                if let Some(color) = color {
                    rgba.chunks_exact_mut(4).for_each(|rgba| {
                        if rgba[3] > 0 {
                            rgba[0] = color[0];
                            rgba[1] = color[1];
                            rgba[2] = color[2];
                        }
                    });
                }

                let allocation =
                    atlas.upload(device, encoder, belt, size.width, size.height, &rgba)?;

                log::debug!("allocating {id} {}x{}", size.width, size.height);

                let _ = self.svg_hits.insert(id);
                let _ = self.rasterized_hits.insert(key);
                let _ = self.rasterized.insert(key, allocation);
                self.should_trim = true;

                self.rasterized.get(&key)
            }
            Svg::NotFound => None,
        }
    }

    /// Load svg and upload raster data
    pub fn trim(&mut self, atlas: &mut Atlas) {
        if !self.should_trim {
            return;
        }

        let svg_hits = &self.svg_hits;
        let rasterized_hits = &self.rasterized_hits;

        self.svgs.retain(|k, _| svg_hits.contains(k));
        self.rasterized.retain(|k, entry| {
            let retain = rasterized_hits.contains(k);

            if !retain {
                atlas.remove(entry);
            }

            retain
        });
        self.svg_hits.clear();
        self.rasterized_hits.clear();
        self.should_trim = false;
    }
}

impl std::fmt::Debug for Svg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Svg::Loaded(_) => write!(f, "Svg::Loaded"),
            Svg::NotFound => write!(f, "Svg::NotFound"),
        }
    }
}

/// Preserve the widget's resolved physical dimensions without a second
/// integer aspect-ratio calculation. Source viewports can be fractional too.
fn raster_transform(tree: &usvg::Tree, target: Size<u32>) -> tiny_skia::Transform {
    tiny_skia::Transform::from_scale(
        target.width as f32 / tree.size().width(),
        target.height as f32 / tree.size().height(),
    )
}

fn straight_rgba(image: tiny_skia::Pixmap) -> Vec<u8> {
    let mut rgba = image.take();
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 && alpha < 255 {
            for component in &mut pixel[..3] {
                *component = ((u32::from(*component) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translucent_svg_edges_are_not_premultiplied_twice() {
        let mut image = tiny_skia::Pixmap::new(1, 1).unwrap();
        image.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 128));
        // Half coverage of white must stay white in the atlas, with alpha
        // carrying coverage. The image shader multiplies by this alpha once.
        assert_eq!(straight_rgba(image), [255, 255, 255, 128]);
    }

    #[test]
    fn svg_edges_land_at_resolved_pixels_instead_of_a_second_aspect_fit() {
        // A wide logo-like viewport at a fractional display scale. The old
        // height-based integer fit made it 269 px wide inside a 267 px texture.
        let tree = usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="49"><path fill="white" d="M196 0h4v49h-4z"/></svg>"#,
            &usvg::Options::default(),
        ).unwrap();
        let target = Size::new(267, 66);
        let mut pixels = tiny_skia::Pixmap::new(target.width, target.height).unwrap();
        resvg::render(&tree, raster_transform(&tree, target), &mut pixels.as_mut());
        let occupied: Vec<_> = (0..target.width)
            .filter(|x| pixels.pixel(*x, 33).unwrap().alpha() > 0)
            .collect();
        assert_eq!(occupied.first(), Some(&261));
        assert_eq!(occupied.last(), Some(&266));
        assert_eq!(pixels.pixel(266, 33).unwrap().alpha(), 255);
    }
}
