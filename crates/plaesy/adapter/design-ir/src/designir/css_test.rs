use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{build_theme, CssError, Theme, ThemeMode};

fn doc() -> Value {
    json!({
        "ir": "design-ir/1",
        "id": "dsg_theme",
        "tokens": {
            "color": {
                "primary": { "$type": "color", "$value": "#2563EB" },
                "surface": { "$type": "color", "$value": "#FFFFFF" },
                "accent": { "$type": "color", "$value": "{color.primary}" }
            },
            "space": {
                "md": { "$type": "dimension", "$value": "16px" }
            },
            "font": {
                "body": {
                    "$type": "typography",
                    "$value": { "fontFamily": "Inter Tight", "fontSize": "14px", "fontWeight": 400 }
                }
            },
            "shadow": {
                "card": {
                    "$type": "shadow",
                    "$value": {
                        "color": "rgba(0,0,0,0.1)",
                        "offsetX": "0px",
                        "offsetY": "1px",
                        "blur": "2px"
                    }
                }
            }
        }
    })
}

fn dark_surface() -> BTreeMap<String, Value> {
    BTreeMap::from([("color.surface".to_string(), json!("#0B1220"))])
}

fn theme(doc: &Value, dark: &BTreeMap<String, Value>) -> Theme {
    build_theme(doc, dark).expect("tema valid")
}

#[test]
fn maps_colour_dimension_and_alias_names() {
    let t = theme(&doc(), &BTreeMap::new());
    let light = t.light();
    assert_eq!(
        light.get("--color-primary").map(String::as_str),
        Some("#2563EB")
    );
    assert_eq!(light.get("--space-md").map(String::as_str), Some("16px"));
    assert_eq!(
        light.get("--color-accent").map(String::as_str),
        Some("var(--color-primary)")
    );
}

#[test]
fn expands_typography_and_shadow() {
    let t = theme(&doc(), &BTreeMap::new());
    let light = t.light();
    // Keluarga dengan spasi dikutip; bobot angka ditulis sebagai teks.
    assert_eq!(
        light.get("--font-body-family").map(String::as_str),
        Some("\"Inter Tight\"")
    );
    assert_eq!(
        light.get("--font-body-size").map(String::as_str),
        Some("14px")
    );
    assert_eq!(
        light.get("--font-body-weight").map(String::as_str),
        Some("400")
    );
    // Tanpa spread, bayangan berisi empat bagian.
    assert_eq!(
        light.get("--shadow-card").map(String::as_str),
        Some("0px 1px 2px rgba(0,0,0,0.1)")
    );
}

#[test]
fn light_block_is_root_and_sorted() {
    let css = theme(&doc(), &BTreeMap::new()).css();
    assert!(css.starts_with(":root {\n"), "{css}");
    let names: Vec<&str> = css
        .lines()
        .filter_map(|l| l.trim().strip_prefix("--"))
        .map(|l| l.split(':').next().unwrap_or(""))
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
    assert!(
        !css.contains("@media"),
        "tanpa override tidak ada blok gelap: {css}"
    );
}

#[test]
fn dark_override_only_in_dark_blocks() {
    let t = theme(&doc(), &dark_surface());
    assert_eq!(
        t.light().get("--color-surface").map(String::as_str),
        Some("#FFFFFF")
    );
    assert_eq!(
        t.dark().get("--color-surface").map(String::as_str),
        Some("#0B1220")
    );
    // Hanya token yang ditimpa masuk ke peta gelap.
    assert_eq!(t.dark().len(), 1);

    let css = t.css();
    let (light_part, dark_part) = css.split_once("@media").expect("ada blok gelap");
    assert!(light_part.contains("--color-surface: #FFFFFF;"));
    assert!(!light_part.contains("#0B1220"));
    // Nilai gelap muncul dua kali: di media query dan di data-theme.
    assert_eq!(dark_part.matches("#0B1220").count(), 2);
    assert!(css.contains(":root:not([data-theme=\"light\"])"));
    assert!(css.contains(":root[data-theme=\"dark\"]"));
}

#[test]
fn dark_mode_inherits_untouched_tokens() {
    let t = theme(&doc(), &dark_surface());
    let dark = t.resolved(ThemeMode::Dark);
    assert_eq!(
        dark.get("--color-surface").map(String::as_str),
        Some("#0B1220")
    );
    // Token yang tidak ditimpa mewarisi nilai terang.
    assert_eq!(
        dark.get("--color-primary").map(String::as_str),
        Some("#2563EB")
    );
    assert!(!t.dark().contains_key("--color-primary"));

    let light = t.resolved(ThemeMode::Light);
    assert_eq!(
        light.get("--color-surface").map(String::as_str),
        Some("#FFFFFF")
    );
}

#[test]
fn shadow_with_spread_keeps_order() {
    let doc = json!({
        "tokens": {
            "shadow": {
                "lift": {
                    "$type": "shadow",
                    "$value": {
                        "color": "#000",
                        "offsetX": "0px",
                        "offsetY": "4px",
                        "blur": "8px",
                        "spread": "-2px"
                    }
                }
            }
        }
    });
    let t = theme(&doc, &BTreeMap::new());
    assert_eq!(
        t.light().get("--shadow-lift").map(String::as_str),
        Some("0px 4px 8px -2px #000")
    );
}

#[test]
fn colliding_variable_names_are_rejected() {
    // `a-b` dan `a.b` sama-sama menjadi `--color-a-b`.
    let doc = json!({
        "tokens": {
            "color": {
                "a-b": { "$type": "color", "$value": "#111" },
                "a": { "b": { "$type": "color", "$value": "#222" } }
            }
        }
    });
    let result = build_theme(&doc, &BTreeMap::new());
    assert_eq!(result, Err(CssError::Collision("--color-a-b".to_string())));
}

#[test]
fn override_for_unknown_token_is_rejected() {
    let dark = BTreeMap::from([("color.missing".to_string(), json!("#000"))]);
    let result = build_theme(&doc(), &dark);
    assert_eq!(
        result,
        Err(CssError::UnknownToken("color.missing".to_string()))
    );
}

#[test]
fn value_that_could_break_out_of_declaration_is_rejected() {
    let doc = json!({
        "tokens": {
            "color": {
                "evil": { "$type": "color", "$value": "red; } body { display: none" }
            }
        }
    });
    let result = build_theme(&doc, &BTreeMap::new());
    assert!(
        matches!(result, Err(CssError::Malformed(ref name, _)) if name == "color.evil"),
        "{result:?}"
    );
}

#[test]
fn typography_missing_field_is_rejected() {
    let doc = json!({
        "tokens": {
            "font": {
                "bad": {
                    "$type": "typography",
                    "$value": { "fontFamily": "Inter", "fontSize": "14px" }
                }
            }
        }
    });
    let result = build_theme(&doc, &BTreeMap::new());
    assert!(
        matches!(result, Err(CssError::Malformed(_, _))),
        "{result:?}"
    );
}
