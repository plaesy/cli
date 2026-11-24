use super::*;

#[test]
fn viewport_zoom_keeps_point_under_cursor_fixed() {
    let mut v = Viewport::default();
    v.pan(10.0, 20.0);
    let before = v.to_world(100.0, 50.0);
    v.zoom_at(100.0, 50.0, 2.0);
    let after = v.to_world(100.0, 50.0);
    assert!((before.0 - after.0).abs() < 1e-9);
    assert!((before.1 - after.1).abs() < 1e-9);
}

#[test]
fn viewport_zoom_is_clamped() {
    let mut v = Viewport::default();
    for _ in 0..50 {
        v.zoom_at(0.0, 0.0, 2.0);
    }
    assert_eq!(v.zoom, MAX_ZOOM);
    for _ in 0..200 {
        v.zoom_at(0.0, 0.0, 0.5);
    }
    assert_eq!(v.zoom, MIN_ZOOM);
}

#[test]
fn viewport_ignores_invalid_zoom_factor() {
    let mut v = Viewport::default();
    v.zoom_at(0.0, 0.0, 0.0);
    v.zoom_at(0.0, 0.0, f64::NAN);
    assert_eq!(v, Viewport::default());
}

#[test]
fn viewport_screen_and_world_round_trip() {
    let v = Viewport {
        zoom: 2.0,
        pan_x: 5.0,
        pan_y: -3.0,
    };
    let (sx, sy) = v.to_screen(4.0, 6.0);
    assert_eq!(v.to_world(sx, sy), (4.0, 6.0));
}
