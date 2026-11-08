# Local Iced GPU renderer correction

Upstream: https://github.com/iced-rs/iced/tree/0d12e964da8c3f0f7a02d3d731e3187eecfe5779/wgpu
License: MIT (included as LICENSE).

Only `src/image/vector.rs` changes renderer behavior. SVG widget layout already
resolves the physical target dimensions. The upstream rasterizer performed an
additional integer aspect fit from the target height for landscape images,
which changed the effective scale and could crop right edges. It also rounded
fractional source dimensions to integers. Rasterize directly from the original
floating source size to the exact allocated texture dimensions instead.

The same path also uploaded tiny-skia's premultiplied RGBA into the shared image
atlas, whose shader expects straight alpha and premultiplies it itself. Convert
the SVG pixels to straight RGBA before upload so antialiased glyph edges retain
their coverage instead of becoming darker/thinner. Raster images and shaders
are unchanged.

The regression renders an edge marker at a fractional display ratio and checks
its actual pixel coverage; the upstream aspect fit moves/crops that marker.
Run from the repository root:

```
cargo test -p iced_wgpu image::vector::tests
```

Cargo.toml expands upstream workspace inheritance and pins the remaining Iced
crates and cryoglyph to their original revisions. No other source files change.
Remove this patch when an upstream revision contains the equivalent correction.
