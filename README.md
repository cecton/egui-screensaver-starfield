[![crates.io](https://img.shields.io/crates/v/egui-screensaver-starfield.svg)](https://crates.io/crates/egui-screensaver-starfield)
[![docs.rs](https://docs.rs/egui-screensaver-starfield/badge.svg)](https://docs.rs/egui-screensaver-starfield)
[![Rust version](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![dependency status](https://deps.rs/repo/github/cecton/egui-screensaver-starfield/status.svg)](https://deps.rs/repo/github/cecton/egui-screensaver-starfield)
[![CI](https://github.com/cecton/egui-screensaver-starfield/actions/workflows/ci.yml/badge.svg)](https://github.com/cecton/egui-screensaver-starfield/actions/workflows/ci.yml)
[![Live Demo](https://img.shields.io/badge/demo-live-brightgreen)](https://cecton.github.io/egui-screensaver-starfield/)

# egui-screensaver-starfield

Starfield screensaver for [egui](https://github.com/emilk/egui).

Renders stars streaking outward from the center of the screen onto the
egui background layer, recreating the classic Windows 95/98 "Starfield
Simulation" screen saver.

## Usage

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
egui-screensaver-starfield = "0.1"
```

Then call `paint` every frame from your `eframe::App::update` implementation,
before drawing any UI windows:

```rust
use egui_screensaver_starfield::StarfieldBackground;

struct MyApp {
    starfield: StarfieldBackground,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.starfield.paint(ctx);
        // … rest of your UI …
    }
}
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
