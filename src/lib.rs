//! Starfield screensaver for [egui](https://github.com/emilk/egui).
//!
//! Renders stars streaking outward from the center of the screen onto the
//! egui background layer, recreating the classic Windows 95/98 "Starfield
//! Simulation" screen saver. The simulation runs at a fixed 30 fps
//! time-step regardless of the actual display refresh rate so the
//! animation looks identical on any monitor. Repaints are capped at 30
//! FPS; if the hardware cannot sustain that rate the screensaver animates
//! as fast as possible without any artificial delay.
//!
//! # Usage
//!
//! ```rust,no_run
//! use egui_screensaver_starfield::StarfieldBackground;
//!
//! struct MyApp {
//!     starfield: StarfieldBackground,
//! }
//!
//! impl eframe::App for MyApp {
//!     fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
//!         let ctx = ui.ctx().clone();
//!         // Call paint once per frame before drawing any UI windows so the
//!         // screensaver sits on the background layer behind everything else.
//!         self.starfield.paint(&ctx);
//!     }
//! }
//! ```

use std::time::Duration;

use egui::{Color32, Context, LayerId, Painter, Pos2, Stroke, emath::lerp, pos2};

// ── Simulation constants ─────────────────────────────────────────────────────

/// Depth at which a star spawns (far away).
const Z_FAR: f32 = 1.0;

/// Depth at which a star passes the camera and respawns.
const Z_NEAR: f32 = 0.035;

/// Minimum spawn radius from the center, in normalised units. Keeps stars
/// from spawning exactly on the vanishing point.
const R_MIN: f32 = 0.03;

/// Depth units travelled per second at `speed = 1.0`. A star takes
/// `(Z_FAR - Z_NEAR) / BASE_Z_UNITS_PER_SEC` seconds (~3.2s) to cross from
/// spawn to camera.
const BASE_Z_UNITS_PER_SEC: f32 = 0.30;

/// Star count at `density = 0.0`.
const MIN_STARS: usize = 120;

/// Star count at `density = 1.0`.
const MAX_STARS: usize = 1800;

/// Fraction of stars given a pale-blue tint instead of white.
const BLUE_TINT_PROBABILITY: f32 = 0.15;

const PALE_BLUE_TINT: Color32 = Color32::from_rgb(0xB8, 0xD4, 0xFF);

/// Target simulation rate. Physics steps always advance by `1/TARGET_FPS`
/// seconds of virtual time, decoupled from the actual rendering rate.
const TARGET_FPS: f64 = 30.0;

/// Virtual time (seconds) consumed by one simulation step.
const TARGET_FRAME_TIME: f64 = 1.0 / TARGET_FPS;

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Advances a small xorshift64 PRNG state, returning the new value. No
/// external `rand` dependency needed for this crate's modest randomness
/// needs (star spawn position and tint).
fn next_u64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Returns a pseudo-random `f32` in `[0, 1)`.
fn next_f32(state: &mut u64) -> f32 {
    (next_u64(state) >> 40) as f32 / (1u64 << 24) as f32
}

/// One star travelling from `Z_FAR` (spawn) toward `Z_NEAR` (the camera).
#[derive(Debug, Clone, Copy)]
struct Star {
    /// Position in a normalised disk, roughly `[-1, 1] × [-1, 1]`.
    x: f32,
    y: f32,
    /// Depth: `Z_FAR` (just spawned) down to `Z_NEAR` (passing the camera).
    z: f32,
    /// Screen position this star was projected to on the previous frame it
    /// was rendered at, used to draw the warp streak. `None` right after
    /// (re)spawn, so a fresh star draws as a dot instead of a spurious line
    /// from wherever the star it replaced last was.
    prev_screen: Option<Pos2>,
    /// Fixed per-star colour, chosen once at spawn.
    tint: Color32,
}

impl Star {
    /// Spawns a new star at depth `z`, at a random position in a disk
    /// around the center (polar sampling, uniform by area) with a
    /// freshly-rolled tint.
    fn spawn(rng_state: &mut u64, z: f32) -> Self {
        let angle = next_f32(rng_state) * std::f32::consts::TAU;
        let r = R_MIN + (1.0 - R_MIN) * next_f32(rng_state).sqrt();
        let tint = if next_f32(rng_state) < BLUE_TINT_PROBABILITY {
            PALE_BLUE_TINT
        } else {
            Color32::WHITE
        };
        Self {
            x: r * angle.cos(),
            y: r * angle.sin(),
            z,
            prev_screen: None,
            tint,
        }
    }
}

/// Maps `density` (`0.0..=1.0`) to an actual star count.
fn star_count(density: f32) -> usize {
    lerp(MIN_STARS as f32..=MAX_STARS as f32, density.clamp(0.0, 1.0)).round() as usize
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Starfield screensaver state.
///
/// Create one instance (e.g. as a field of your `eframe::App` struct) and
/// call [`StarfieldBackground::paint`] every frame from your `update`
/// method.
#[derive(Debug)]
pub struct StarfieldBackground {
    stars: Vec<Star>,
    /// The `density` value `stars` was last sized for.
    applied_density: f32,
    /// xorshift64 PRNG state.
    rng_state: u64,
    /// Wall-clock time (seconds) at the previous call to [`paint`].
    last_time: Option<f64>,
    /// Accumulated wall-clock time not yet consumed by simulation steps.
    time_accumulator: f64,
    /// Travel-speed multiplier. `1.0` is the default warp speed, `0.0`
    /// freezes the simulation, values above `1.0` go faster.
    pub speed: f32,
    /// Star density knob in `0.0..=1.0`, mapped internally to a star count
    /// between a fixed minimum and maximum.
    pub density: f32,
}

impl Default for StarfieldBackground {
    fn default() -> Self {
        let mut rng_state = 0xD1B5_4A32_D192_ED03_u64;
        let density = 0.4;

        // Spawn every star at a random depth within range (not all at
        // `Z_FAR`) so the very first frame doesn't look like a
        // synchronized burst from the center.
        let stars = (0..star_count(density))
            .map(|_| {
                let z = Z_NEAR + (Z_FAR - Z_NEAR) * next_f32(&mut rng_state);
                Star::spawn(&mut rng_state, z)
            })
            .collect();

        Self {
            stars,
            applied_density: density,
            rng_state,
            last_time: None,
            time_accumulator: 0.0,
            speed: 1.0,
            density,
        }
    }
}

impl StarfieldBackground {
    /// Paint the screensaver onto the egui background layer for this frame.
    ///
    /// Call this once per frame **before** drawing any UI panels or windows
    /// so the animation appears behind all other content.
    ///
    /// Repaints are capped at 30 FPS; if the hardware cannot sustain that
    /// rate the screensaver animates as fast as possible without any
    /// artificial delay.
    pub fn paint(&mut self, ctx: &Context) {
        ctx.request_repaint_after(Duration::from_secs_f64(1.0 / TARGET_FPS));

        let time = ctx.input(|input| input.time);

        // Accumulate wall-clock time elapsed since the last frame, clamped
        // to 250ms so a tab switch or debugger pause doesn't produce a huge
        // jump.
        if let Some(last_time) = self.last_time {
            let elapsed = (time - last_time).clamp(0.0, 0.25);
            self.time_accumulator += elapsed;
        }
        self.last_time = Some(time);

        if self.density != self.applied_density {
            self.resize_stars();
        }

        let step_dt = TARGET_FRAME_TIME as f32 * self.speed.max(0.0);

        while self.time_accumulator >= TARGET_FRAME_TIME {
            let dz = step_dt * BASE_Z_UNITS_PER_SEC;
            for star in &mut self.stars {
                star.z -= dz;
                if star.z <= Z_NEAR {
                    *star = Star::spawn(&mut self.rng_state, Z_FAR);
                }
            }
            self.time_accumulator -= TARGET_FRAME_TIME;
        }

        let rect = ctx.content_rect();
        let painter = Painter::new(ctx.clone(), LayerId::background(), rect);
        let scale = rect.width().min(rect.height()) * 0.5;
        let center = rect.center();

        for star in &mut self.stars {
            let screen_pos = pos2(
                center.x + star.x / star.z * scale,
                center.y + star.y / star.z * scale,
            );

            // 0 when just spawned (far), 1 when about to pass the camera
            // (close) — drives brightness and stroke width so near stars
            // are bright/thick and far stars are dim/thin.
            let proximity = ((star.z - Z_NEAR) / (Z_FAR - Z_NEAR)).clamp(0.0, 1.0);
            let proximity = 1.0 - proximity;
            let alpha = lerp(70.0..=255.0, proximity).round() as u8;
            let color =
                Color32::from_rgba_unmultiplied(star.tint.r(), star.tint.g(), star.tint.b(), alpha);
            let stroke_width = lerp(0.6..=2.6, proximity);

            if let Some(prev) = star.prev_screen {
                painter.line_segment([prev, screen_pos], Stroke::new(stroke_width, color));
            } else {
                painter.circle_filled(screen_pos, 0.6, color);
            }
            star.prev_screen = Some(screen_pos);
        }
    }

    /// Resizes `stars` to match `star_count(self.density)`. Growing adds
    /// new stars at a random depth within range (avoids a synchronized
    /// burst on a density increase); shrinking truncates.
    fn resize_stars(&mut self) {
        let target = star_count(self.density);
        if target > self.stars.len() {
            self.stars.extend((self.stars.len()..target).map(|_| {
                let z = Z_NEAR + (Z_FAR - Z_NEAR) * next_f32(&mut self.rng_state);
                Star::spawn(&mut self.rng_state, z)
            }));
        } else {
            self.stars.truncate(target);
        }
        self.applied_density = self.density;
    }
}
