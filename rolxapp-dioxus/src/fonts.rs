//! The Cudos brand font (KMR Apparat), loaded from a design system checkout when there is one.
//!
//! The font files are licensed separately from the design system and must not be committed here.
//! Without them the stylesheet falls back to Helvetica Neue / Arial, as the design system does.

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// (CSS weight, file name). Medium carries titles and labels, Bold the metric numbers.
const FACES: [(u16, &str); 3] = [
    (400, "KMR-Apparat-Regular.woff2"),
    (500, "KMR-Apparat-Medium.woff2"),
    (700, "KMR-Apparat-Bold.woff2"),
];

/// `ROLX_FONT_DIR`, else `public/fonts` of the `./design-system` checkout (`cargo xtask fetch-design-system`).
fn font_dir() -> PathBuf {
    std::env::var_os("ROLX_FONT_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../design-system/public/fonts"),
        PathBuf::from,
    )
}

/// `@font-face` rules with the font files embedded, or an empty string when they are not available.
pub fn font_face_css() -> String {
    let dir = font_dir();
    FACES
        .iter()
        .filter_map(|(weight, file)| {
            let bytes = std::fs::read(dir.join(file)).ok()?;
            Some(format!(
                "@font-face {{ font-family: 'KMR Apparat'; font-style: normal; font-weight: {weight}; font-display: swap; src: url(data:font/woff2;base64,{}) format('woff2'); }}\n",
                STANDARD.encode(bytes)
            ))
        })
        .collect()
}
