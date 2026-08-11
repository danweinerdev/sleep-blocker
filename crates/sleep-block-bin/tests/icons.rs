//! Guards on the bundled tray icons.
//!
//! These exist because a mismatch here fails silently: the tray decoder
//! rejects any icon that is not 8-bit RGBA and simply publishes no pixmap, so
//! the tray shows a blank or stale icon while everything else works. That is
//! easy to miss by eye and impossible to spot from a passing build.

use std::path::{Path, PathBuf};

fn icon_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/icons")
}

/// Every icon the tray embeds, as (state, size) pairs.
fn embedded_icons() -> Vec<PathBuf> {
    let dir = icon_dir();
    ["active", "idle"]
        .iter()
        .flat_map(|state| {
            [22, 32, 48]
                .iter()
                .map(|size| dir.join(format!("sleep-block-{state}-{size}.png")))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn embedded_icons_are_eight_bit_rgba() {
    for path in embedded_icons() {
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("missing icon {}: {e}", path.display()));
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let reader = decoder
            .read_info()
            .unwrap_or_else(|e| panic!("unreadable icon {}: {e}", path.display()));
        let info = reader.info();

        // ImageMagick writes 16-bit PNGs by default, which the tray decoder
        // rejects; the generation step must force 8-bit RGBA.
        assert_eq!(
            info.bit_depth,
            png::BitDepth::Eight,
            "{} must be 8-bit, got {:?}",
            path.display(),
            info.bit_depth
        );
        assert_eq!(
            info.color_type,
            png::ColorType::Rgba,
            "{} must be RGBA, got {:?}",
            path.display(),
            info.color_type
        );
    }
}

#[test]
fn embedded_icons_have_expected_dimensions() {
    for path in embedded_icons() {
        let expected: u32 = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('-').next())
            .and_then(|s| s.parse().ok())
            .expect("icon filename should end in its pixel size");

        let file = std::fs::File::open(&path).expect("icon should exist");
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let reader = decoder.read_info().expect("icon should decode");
        let info = reader.info();

        assert_eq!(
            (info.width, info.height),
            (expected, expected),
            "{} should be {expected}x{expected}",
            path.display()
        );
    }
}

/// The two states must not render identically, or the tray icon would convey
/// nothing. Comparing raw bytes is enough: they are generated from different
/// source SVGs.
#[test]
fn active_and_idle_icons_differ() {
    let dir = icon_dir();
    for size in [22, 32, 48] {
        let active = std::fs::read(dir.join(format!("sleep-block-active-{size}.png")))
            .expect("active icon should exist");
        let idle = std::fs::read(dir.join(format!("sleep-block-idle-{size}.png")))
            .expect("idle icon should exist");
        assert_ne!(
            active, idle,
            "active and idle icons at {size}px must be visually distinct"
        );
    }
}
