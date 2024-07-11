2D rendering library, intended for user interfaces.

# Features

- Can draw rectangles with rounded corners, borders, and shadows.

- Can draw text using various rasterization backends (freetype, zeno).

- Can fill/stroke vector paths using triangulation through lyon.

- Supports layers for implementing things such as group transparency or tint

- Everything is pluggable: you can implement your own `Renderer`,
  `TextShaper`, `FontDatabase`, `FontRasterizer`, `AssetSource`,
  `ImageDecoder`.

# Philosophy

The philosophy of Ohm is to optimize rendering for the most common
primitives in modern UI: rounded rectangles with borders, shadows and text.
Ohm is not a general purpose vector graphics renderer. It's not intended for
rendering high quality SVG images (for that you'd use resvg, which is
integrated with Ohm through `resvg` feature)

The default renderer can draw a textured rectangle with rounded corners,
borders and a shadow using a single quad. It can also draw text using the
same ubershader, minimizing the number draw calls. The renderer doesn't
require any special hardware features, such as compute shaders, so should
run anywhere.

# External dependencies and cargo features

- Enable `fontdb` feature for access to system fonts (backed by `fontdb`).

- Enable `freetype` feature for the FreeType font renderer

- Enable `image` feature for decoding raster images (backed by `image`).
  This also includes raster images, embedded in fonts (such as emoji).
  You can also enable specific formats using features, such as `image/png`,
  `image/jpeg`.

 - Enable `resvg` feature for rendering SVG images (backed by `resvg`).

 - Enable `rustybuzz` feature for text shaping (backed by `rustybuzz`).

 - Enable `wgpu` feature for the default renderer implementation (backed by
   `wgpu`).

 - Enable `zeno` feature for a portable font renderer (backed by `zeno`).

By default, all features are enabled.


# License

This project is licensed under either of

- Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

# Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
