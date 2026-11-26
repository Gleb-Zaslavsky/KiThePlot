//! Publication metadata helpers for image exports.

use crate::model::PublicationDimensions;

pub(crate) fn set_svg_physical_size(
    svg: &str,
    dimensions: PublicationDimensions,
) -> Result<String, String> {
    let (width_mm, height_mm) = dimensions.size_in_millimeters();
    let start = svg
        .find("<svg")
        .ok_or_else(|| "SVG document has no root element".to_owned())?;
    let end = svg[start..]
        .find('>')
        .map(|offset| start + offset + 1)
        .ok_or_else(|| "SVG root element is not closed".to_owned())?;
    let mut root = svg[start..end].to_owned();
    replace_svg_attribute(&mut root, "width", &format!("{width_mm:.4}mm"))?;
    replace_svg_attribute(&mut root, "height", &format!("{height_mm:.4}mm"))?;
    let mut result = String::with_capacity(svg.len() + 32);
    result.push_str(&svg[..start]);
    result.push_str(&root);
    result.push_str(&svg[end..]);
    Ok(result)
}

fn replace_svg_attribute(root: &mut String, name: &str, value: &str) -> Result<(), String> {
    let key = format!("{name}=\"");
    let value_start = root
        .find(&key)
        .map(|index| index + key.len())
        .ok_or_else(|| format!("SVG root has no {name} attribute"))?;
    let value_end = root[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
        .ok_or_else(|| format!("SVG root has an unterminated {name} attribute"))?;
    root.replace_range(value_start..value_end, value);
    Ok(())
}

pub(crate) fn set_png_dpi(png: &[u8], dpi: u32) -> Result<Vec<u8>, String> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !png.starts_with(SIGNATURE) {
        return Err("PNG export has an invalid signature".to_owned());
    }
    let pixels_per_meter = ((dpi as f64) / 0.0254).round();
    if !pixels_per_meter.is_finite() || pixels_per_meter > u32::MAX as f64 {
        return Err("DPI cannot be represented in PNG metadata".to_owned());
    }
    let ppm = (pixels_per_meter as u32).to_be_bytes();
    let mut chunk = Vec::with_capacity(21);
    chunk.extend_from_slice(&9_u32.to_be_bytes());
    chunk.extend_from_slice(b"pHYs");
    chunk.extend_from_slice(&ppm);
    chunk.extend_from_slice(&ppm);
    chunk.push(1); // pixels per meter
    let checksum = crc32fast::hash(&chunk[4..]);
    chunk.extend_from_slice(&checksum.to_be_bytes());

    let mut offset = SIGNATURE.len();
    while offset + 12 <= png.len() {
        let data_len = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(data_len)
            .and_then(|end| end.checked_add(12))
            .filter(|end| *end <= png.len())
            .ok_or_else(|| "PNG contains a truncated chunk".to_owned())?;
        let kind = &png[offset + 4..offset + 8];
        if kind == b"pHYs" {
            return Err("PNG already contains a physical-resolution chunk".to_owned());
        }
        if kind == b"IHDR" {
            let mut output = Vec::with_capacity(png.len() + chunk.len());
            output.extend_from_slice(&png[..chunk_end]);
            output.extend_from_slice(&chunk);
            output.extend_from_slice(&png[chunk_end..]);
            return Ok(output);
        }
        offset = chunk_end;
    }
    Err("PNG has no complete image header".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_uses_physical_root_dimensions_and_keeps_pixel_viewbox() {
        let dimensions = PublicationDimensions {
            width: 85.0,
            height: 60.0,
            unit: crate::model::PhysicalUnit::Millimeters,
            dpi: 300,
        };
        let svg = set_svg_physical_size(
            r#"<svg width="1000" height="700" viewBox="0 0 1000 700"></svg>"#,
            dimensions,
        )
        .unwrap();
        assert!(svg.contains("width=\"85.0000mm\""));
        assert!(svg.contains("height=\"60.0000mm\""));
        assert!(svg.contains("viewBox=\"0 0 1000 700\""));
    }
}
