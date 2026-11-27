//! Shared chart fonts for Plotters and egui's preview backend.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, Once, OnceLock};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use egui::{Context, FontData, FontFamily, FontInsert, FontPriority, InsertFontFamily};
#[cfg(not(target_arch = "wasm32"))]
use plotters::style::{FontStyle, register_font};
use sha2::{Digest, Sha256};

pub(crate) const SANS: &str = "KiThePlot Sans";
pub(crate) const SERIF: &str = "KiThePlot Serif";
pub(crate) const MONO: &str = "KiThePlot Mono";

static BUILT_INS: Once = Once::new();
static CUSTOM_ASSETS: OnceLock<Mutex<HashMap<String, &'static [u8]>>> = OnceLock::new();
static CUSTOM_LABELS: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
static LEGACY_NAMES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub(crate) fn renderer_family(asset_id: &str) -> String {
    format!("KiThePlot Custom {asset_id}")
}

pub(crate) fn legacy_renderer_family(family: &str) -> String {
    LEGACY_NAMES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("chart font registry poisoned")
        .get(family)
        .map(|asset_id| renderer_family(asset_id))
        .unwrap_or_else(|| format!("KiThePlot Legacy {family}"))
}

pub(crate) fn font_asset_id(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn ensure_plotters_fonts() {
    #[cfg(not(target_arch = "wasm32"))]
    BUILT_INS.call_once(|| {
        register(SANS, include_bytes!("../assets/fonts/NotoSans.ttf"));
        register(SERIF, include_bytes!("../assets/fonts/NotoSerif.ttf"));
        register(MONO, include_bytes!("../assets/fonts/NotoSansMono.ttf"));
    });
    #[cfg(target_arch = "wasm32")]
    let _ = &BUILT_INS;
}

#[cfg(not(target_arch = "wasm32"))]
fn register(name: &str, bytes: &'static [u8]) {
    register_font(name, FontStyle::Normal, bytes)
        .unwrap_or_else(|_| panic!("invalid embedded chart font: {name}"));
}

pub(crate) fn install_builtin_egui_fonts(ctx: &Context) {
    ctx.add_font(FontInsert::new(
        SANS,
        FontData::from_static(include_bytes!("../assets/fonts/NotoSans.ttf")),
        vec![InsertFontFamily {
            family: FontFamily::Name(SANS.into()),
            priority: FontPriority::Highest,
        }],
    ));
    let assets = CUSTOM_ASSETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("chart font registry poisoned")
        .iter()
        .map(|(asset_id, bytes)| (renderer_family(asset_id), *bytes))
        .collect::<Vec<_>>();
    for (family, bytes) in assets {
        ctx.add_font(FontInsert::new(
            family.as_str(),
            FontData::from_static(bytes),
            vec![InsertFontFamily {
                family: FontFamily::Name(family.clone().into()),
                priority: FontPriority::Highest,
            }],
        ));
    }
    ctx.add_font(FontInsert::new(
        SERIF,
        FontData::from_static(include_bytes!("../assets/fonts/NotoSerif.ttf")),
        vec![InsertFontFamily {
            family: FontFamily::Name(SERIF.into()),
            priority: FontPriority::Highest,
        }],
    ));
    ctx.add_font(FontInsert::new(
        MONO,
        FontData::from_static(include_bytes!("../assets/fonts/NotoSansMono.ttf")),
        vec![InsertFontFamily {
            family: FontFamily::Name(MONO.into()),
            priority: FontPriority::Highest,
        }],
    ));
}

/// Registers a host-provided TrueType/OpenType font for chart rendering.
///
/// Returns a stable SHA-256 fingerprint for the exact font bytes. Store that
/// value with the family name in `PlotFontFamily::CustomAsset` to keep documents
/// independent when different files share the same visible family name.
pub fn register_chart_font(
    ctx: &Context,
    family: impl Into<String>,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let family = family.into();
    let asset_id = register_font_asset(&family, bytes)?;
    let bytes = registered_font_asset(&asset_id).expect("registered font asset must exist");
    let renderer_family = renderer_family(&asset_id);
    ctx.add_font(FontInsert::new(
        renderer_family.as_str(),
        FontData::from_static(bytes),
        vec![InsertFontFamily {
            family: FontFamily::Name(renderer_family.clone().into()),
            priority: FontPriority::Highest,
        }],
    ));
    Ok(asset_id)
}

pub(crate) fn register_font_asset(family: &str, bytes: Vec<u8>) -> Result<String, String> {
    ensure_plotters_fonts();
    if family.trim().is_empty()
        || !family
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || " -_".contains(character))
    {
        return Err(
            "font family names may contain only ASCII letters, digits, spaces, '-' and '_'"
                .to_owned(),
        );
    }

    ab_glyph::FontRef::try_from_slice(&bytes)
        .map_err(|_| format!("could not parse font asset for family '{family}'"))?;
    let asset_id = font_asset_id(&bytes);
    let mut assets = CUSTOM_ASSETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("chart font registry poisoned");
    if !assets.contains_key(&asset_id) {
        let static_bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        #[cfg(not(target_arch = "wasm32"))]
        register_font(&renderer_family(&asset_id), FontStyle::Normal, static_bytes)
            .map_err(|_| format!("could not parse font asset for family '{family}'"))?;
        assets.insert(asset_id.clone(), static_bytes);
    }
    drop(assets);

    CUSTOM_LABELS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .expect("chart font registry poisoned")
        .insert((family.to_owned(), asset_id.clone()));
    LEGACY_NAMES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("chart font registry poisoned")
        .insert(family.to_owned(), asset_id.clone());
    Ok(asset_id)
}

pub(crate) fn registered_chart_fonts() -> Vec<(String, String)> {
    let mut fonts: Vec<_> = CUSTOM_LABELS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .expect("chart font registry poisoned")
        .iter()
        .cloned()
        .collect();
    fonts.sort();
    fonts
}

pub(crate) fn registered_font_asset(asset_id: &str) -> Option<&'static [u8]> {
    CUSTOM_ASSETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("chart font registry poisoned")
        .get(asset_id)
        .copied()
}

pub(crate) fn font_asset_by_legacy_name(family: &str) -> Option<(String, &'static [u8])> {
    let asset_id = LEGACY_NAMES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("chart font registry poisoned")
        .get(family)
        .cloned()?;
    Some((asset_id.clone(), registered_font_asset(&asset_id)?))
}

pub(crate) fn embed_font_in_svg(svg: &str, family: &str) -> Result<String, String> {
    let bytes = font_bytes_for_renderer_family(family)?;
    let encoded = BASE64.encode(bytes);
    let style = format!(
        "<style>@font-face{{font-family:'{family}';src:url(data:font/ttf;base64,{encoded}) format('truetype');font-weight:100 900;font-style:normal;}}</style>"
    );
    let root = svg
        .find("<svg")
        .ok_or_else(|| "SVG document has no root element".to_owned())?;
    let insertion = svg[root..]
        .find('>')
        .map(|offset| root + offset + 1)
        .ok_or_else(|| "SVG root element is not closed".to_owned())?;
    let mut result = String::with_capacity(svg.len() + style.len());
    result.push_str(&svg[..insertion]);
    result.push_str(&style);
    result.push_str(&svg[insertion..]);
    Ok(result)
}

pub(crate) fn font_bytes_for_renderer_family(family: &str) -> Result<&'static [u8], String> {
    match family {
        SANS => Ok(include_bytes!("../assets/fonts/NotoSans.ttf")),
        SERIF => Ok(include_bytes!("../assets/fonts/NotoSerif.ttf")),
        MONO => Ok(include_bytes!("../assets/fonts/NotoSansMono.ttf")),
        custom if custom.starts_with("KiThePlot Custom ") => {
            let asset_id = &custom["KiThePlot Custom ".len()..];
            registered_font_asset(asset_id)
                .ok_or_else(|| format!("font asset '{asset_id}' has not been registered"))
        }
        legacy => Err(format!("font family '{legacy}' has not been registered")),
    }
}
