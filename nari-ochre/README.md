# nari-ochre

Is a fork of the `ochre` crate adapted to the rendering library used in `nari`.

# ochre

[![Cargo](https://img.shields.io/crates/v/ochre.svg)](https://crates.io/crates/ochre)
[![Documentation](https://docs.rs/ochre/badge.svg)](https://docs.rs/ochre)

High-quality anti-aliased vector graphics rendering on the GPU.

`ochre` rasterizes a path to a set of 8×8-pixel alpha-mask tiles at the path's boundary and n×8-pixel solid spans for the path's interior, which can then be uploaded to the GPU and rendered. Paths are rasterized using a high-quality analytic anti-aliasing method suitable for both text and general vector graphics.

## License

`ochre` is distributed under the terms of both the [MIT license](LICENSE-MIT) and the [Apache license, version 2.0](LICENSE-APACHE). Contributions are accepted under the same terms.
