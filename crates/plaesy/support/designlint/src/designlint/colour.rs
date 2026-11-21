//! `colour` helpers split out of `designlint.rs`.

use super::*;

/// An sRGB colour, channels 0..255.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rgb {
    r: f64,
    g: f64,
    b: f64,
}

/// A colour with alpha 0..1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Colour {
    pub(super) c: Rgb,
    pub(super) a: f64,
}

impl Colour {
    /// Key used to detect conflicting declarations of one token.
    pub(super) fn key(self) -> String {
        format!(
            "{:02x}{:02x}{:02x}/{:.3}",
            self.c.r.round() as u8,
            self.c.g.round() as u8,
            self.c.b.round() as u8,
            self.a
        )
    }

    /// `#rrggbb`, with the opacity appended when it is not 1.
    pub(super) fn hex(self) -> String {
        let mut s = format!(
            "#{:02x}{:02x}{:02x}",
            self.c.r.round() as u8,
            self.c.g.round() as u8,
            self.c.b.round() as u8
        );
        if self.a < 1.0 {
            s.push_str(&format!(" at {:.0}% opacity", self.a * 100.0));
        }
        s
    }
}

fn hex_colour_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?-u:\b)")
            .expect("static regex")
    })
}

fn rgb_colour_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)rgba?\(\s*([0-9.]+)\s*[, ]\s*([0-9.]+)\s*[, ]\s*([0-9.]+)\s*(?:[,/]\s*([0-9.]+)(%?)\s*)?\)")
            .expect("static regex")
    })
}

/// Parses the first colour in `value`: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
/// `rgb()`/`rgba()` with 0..255 channels, or one of the named keywords.
pub(super) fn parse_colour(value: &str) -> Option<Colour> {
    let value = value.trim();
    if let Some(m) = hex_colour_re().captures(value) {
        let mut hex = m[1].to_owned();
        if hex.len() == 3 || hex.len() == 4 {
            hex = hex.chars().flat_map(|c| [c, c]).collect();
        }
        let n = u64::from_str_radix(&hex, 16).ok()?;
        if hex.len() == 8 {
            return Some(Colour {
                c: Rgb {
                    r: ((n >> 24) & 0xff) as f64,
                    g: ((n >> 16) & 0xff) as f64,
                    b: ((n >> 8) & 0xff) as f64,
                },
                a: (n & 0xff) as f64 / 255.0,
            });
        }
        return Some(Colour {
            c: Rgb {
                r: ((n >> 16) & 0xff) as f64,
                g: ((n >> 8) & 0xff) as f64,
                b: (n & 0xff) as f64,
            },
            a: 1.0,
        });
    }
    if let Some(m) = rgb_colour_re().captures(value) {
        let mut channels = [0.0f64; 3];
        for (slot, cap) in channels.iter_mut().zip(1..=3) {
            let v: f64 = m[cap].parse().ok()?;
            if !(0.0..=255.0).contains(&v) {
                return None;
            }
            *slot = v;
        }
        let mut alpha = 1.0;
        if let Some(a) = m.get(4) {
            let mut a: f64 = a.as_str().parse().ok()?;
            if m.get(5).is_some_and(|p| p.as_str() == "%") {
                a /= 100.0;
            }
            alpha = a.clamp(0.0, 1.0);
        }
        return Some(Colour {
            c: Rgb {
                r: channels[0],
                g: channels[1],
                b: channels[2],
            },
            a: alpha,
        });
    }
    match value.to_lowercase().as_str() {
        "black" => Some(Colour {
            c: Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            a: 1.0,
        }),
        "white" => Some(Colour {
            c: Rgb {
                r: 255.0,
                g: 255.0,
                b: 255.0,
            },
            a: 1.0,
        }),
        "transparent" => Some(Colour {
            c: Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            a: 0.0,
        }),
        _ => None,
    }
}

/// Draws `fg` over an opaque `bg`.
pub(super) fn composite(fg: Colour, bg: Colour) -> Rgb {
    Rgb {
        r: fg.c.r * fg.a + bg.c.r * (1.0 - fg.a),
        g: fg.c.g * fg.a + bg.c.g * (1.0 - fg.a),
        b: fg.c.b * fg.a + bg.c.b * (1.0 - fg.a),
    }
}

/// WCAG 2.x relative luminance.
fn luminance(c: Rgb) -> f64 {
    let lin = |v: f64| {
        let s = v / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two opaque colours, from 1 to 21.
pub(super) fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (mut la, mut lb) = (luminance(a), luminance(b));
    if la < lb {
        std::mem::swap(&mut la, &mut lb);
    }
    (la + 0.05) / (lb + 0.05)
}
