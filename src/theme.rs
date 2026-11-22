//! Design tokens: one place for Bloop's colours, radii and timings.
use std::time::Duration;
use zgx::scene::Color;

pub const STAGE: Color = Color(9, 9, 11, 255);
pub const GLASS: Color = Color(22, 22, 28, 168);
pub const GLASS_STRONG: Color = Color(24, 24, 30, 236);
pub const GLASS_BORDER: Color = Color(255, 255, 255, 22);
pub const HOVER: Color = Color(255, 255, 255, 20);
pub const PRESS: Color = Color(255, 255, 255, 34);
pub const TEXT: Color = Color(238, 238, 244, 255);
pub const TEXT_DIM: Color = Color(160, 160, 172, 255);
pub const TEXT_FAINT: Color = Color(110, 110, 122, 255);
pub const ACCENT: Color = Color(139, 92, 246, 255);
pub const ACCENT_SOFT: Color = Color(139, 92, 246, 60);
pub const CORAL: Color = Color(255, 122, 89, 255);
pub const DANGER: Color = Color(239, 68, 68, 255);
pub const CLOSE_HOVER: Color = Color(196, 43, 28, 255);
pub const SHADOW: Color = Color(0, 0, 0, 140);

pub const TITLEBAR_H: f32 = 40.;
pub const THUMB: f32 = 56.;
pub const INFO_W: f32 = 320.;
pub const RADIUS: f32 = 12.;
pub const RADIUS_SM: f32 = 8.;

/// How long the chrome stays after the pointer stops moving.
pub const CHROME_IDLE: Duration = Duration::from_millis(1800);
/// Fade in/out of chrome and cross-fade between items.
pub const FADE_MS: f32 = 160.;
pub const SLIDESHOW: Duration = Duration::from_secs(4);
