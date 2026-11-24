//! Viewport: zoom dan pan. Koordinat dunia dan layar dipisah; zoom menjaga titik di bawah kursor.

pub const MIN_ZOOM: f64 = 0.1;
pub const MAX_ZOOM: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }
}

impl Viewport {
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    /// Zoom dengan faktor di titik layar (sx, sy). Titik dunia di bawah titik itu tetap di tempatnya.
    /// Faktor non-positif atau bukan angka diabaikan.
    pub fn zoom_at(&mut self, sx: f64, sy: f64, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let (wx, wy) = self.to_world(sx, sy);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan_x = sx - wx * self.zoom;
        self.pan_y = sy - wy * self.zoom;
    }

    #[must_use]
    pub fn to_screen(&self, wx: f64, wy: f64) -> (f64, f64) {
        (wx * self.zoom + self.pan_x, wy * self.zoom + self.pan_y)
    }

    #[must_use]
    pub fn to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        ((sx - self.pan_x) / self.zoom, (sy - self.pan_y) / self.zoom)
    }
}

#[cfg(test)]
#[path = "viewport_test.rs"]
mod viewport_test;
