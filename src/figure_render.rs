//! Optional native static export backend. One canonical SVG/tree/bitmap at a time.
use crate::cli::FigureFormat;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use resvg::{tiny_skia, usvg};
use std::{
    borrow::Cow,
    io::{Seek, SeekFrom, Write},
};

const SVG_DPI: f32 = 96.0;
pub const RASTER_DPI: u32 = 300;
const PIXELS_PER_METER: u32 = 11_811; // 300 / 0.0254, rounded.
const MAX_PIXELS: u64 = 64_000_000;

pub struct Renderer {
    options: usvg::Options<'static>,
}

impl Renderer {
    pub fn new() -> Self {
        let mut options = usvg::Options {
            dpi: SVG_DPI,
            font_family: "Liberation Sans".into(),
            ..usvg::Options::default()
        };
        let fonts = options.fontdb_mut();
        fonts.load_font_data(include_bytes!("../assets/fonts/LiberationSans-Regular.ttf").to_vec());
        fonts.load_font_data(include_bytes!("../assets/fonts/LiberationSans-Bold.ttf").to_vec());
        fonts.set_sans_serif_family("Liberation Sans");
        // Generated SVGs contain only data URIs/internal references. Never load
        // an image from a filesystem path or network URL, even accidentally.
        options.image_href_resolver.resolve_string = Box::new(|_, _| None);
        Self { options }
    }

    pub fn write(
        &self,
        svg: &str,
        format: FigureFormat,
        writer: &mut (impl Write + Seek),
    ) -> Result<()> {
        let svg = atlas_tiles(svg)?;
        let tree =
            usvg::Tree::from_str(&svg, &self.options).context("cannot parse canonical SVG")?;
        if matches!(format, FigureFormat::Pdf) {
            let bytes = svg2pdf::to_pdf(
                &tree,
                svg2pdf::ConversionOptions::default(),
                svg2pdf::PageOptions { dpi: SVG_DPI },
            )
            .map_err(|e| anyhow::anyhow!("SVG-to-PDF conversion failed: {e}"))?;
            writer.write_all(&bytes)?;
            return Ok(());
        }
        let size = tree.size();
        let width = (size.width() * RASTER_DPI as f32 / SVG_DPI).round() as u32;
        let height = (size.height() * RASTER_DPI as f32 / SVG_DPI).round() as u32;
        ensure!(
            width > 0 && height > 0 && u64::from(width) * u64::from(height) <= MAX_PIXELS,
            "figure requires {width}×{height} pixels at {RASTER_DPI} DPI; maximum is {MAX_PIXELS} pixels; use vector SVG/PDF or --smallfigures"
        );
        let mut pixmap =
            tiny_skia::Pixmap::new(width, height).context("cannot allocate figure bitmap")?;
        pixmap.fill(tiny_skia::Color::WHITE);
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(
                width as f32 / size.width(),
                height as f32 / size.height(),
            ),
            &mut pixmap.as_mut(),
        );
        // White compositing makes every pixel opaque. Compact RGBA to RGB in
        // the same allocation, rather than keeping a second full bitmap.
        let mut rgb = pixmap.take();
        for i in 0..width as usize * height as usize {
            rgb.copy_within(i * 4..i * 4 + 3, i * 3);
        }
        rgb.truncate(width as usize * height as usize * 3);
        match format {
            FigureFormat::Png => {
                let mut encoder = png::Encoder::new(writer, width, height);
                encoder.set_color(png::ColorType::Rgb);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_pixel_dims(Some(png::PixelDimensions {
                    xppu: PIXELS_PER_METER,
                    yppu: PIXELS_PER_METER,
                    unit: png::Unit::Meter,
                }));
                encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
                let mut output = encoder.write_header()?;
                output.write_image_data(&rgb)?;
                output.finish()?;
            }
            FigureFormat::Jpeg => {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 95);
                encoder
                    .set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(RASTER_DPI as u16));
                encoder.encode(&rgb, width, height, image::ExtendedColorType::Rgb8)?;
            }
            FigureFormat::Tiff => {
                let mut encoder = tiff::encoder::TiffEncoder::new(writer)?.with_compression(
                    tiff::encoder::Compression::Deflate(tiff::encoder::DeflateLevel::Fast),
                );
                let mut output =
                    encoder.new_image::<tiff::encoder::colortype::RGB8>(width, height)?;
                // Use fallible tag writes: convenience resolution() methods
                // unwrap I/O errors, which would defeat staging cleanup.
                output
                    .encoder()
                    .write_tag(tiff::tags::Tag::ResolutionUnit, 2u16)?;
                let dpi = tiff::encoder::Rational {
                    n: RASTER_DPI,
                    d: 1,
                };
                output
                    .encoder()
                    .write_tag(tiff::tags::Tag::XResolution, dpi.clone())?;
                output
                    .encoder()
                    .write_tag(tiff::tags::Tag::YResolution, dpi)?;
                output.write_data(&rgb)?;
            }
            FigureFormat::Bmp => {
                let start = writer.stream_position()?;
                image::codecs::bmp::BmpEncoder::new(&mut *writer).encode(
                    &rgb,
                    width,
                    height,
                    image::ExtendedColorType::Rgb8,
                )?;
                let end = writer.stream_position()?;
                // BITMAPINFOHEADER X/Y pixels-per-meter fields (file offsets
                // 38/42). The image encoder otherwise writes zero resolution.
                writer.seek(SeekFrom::Start(start + 38))?;
                writer.write_all(&PIXELS_PER_METER.to_le_bytes())?;
                writer.write_all(&PIXELS_PER_METER.to_le_bytes())?;
                writer.seek(SeekFrom::Start(end))?;
            }
            FigureFormat::Ps => postscript(
                writer,
                &rgb,
                width,
                height,
                size.width() * 72.0 / SVG_DPI,
                size.height() * 72.0 / SVG_DPI,
            )?,
            _ => anyhow::bail!("{} is not a static conversion format", format.extension()),
        }
        Ok(())
    }
}

/// Preserve the canonical MinION SVG geometry but crop its shared atlas into
/// equivalent per-panel image tiles. resvg otherwise decodes/transforms the
/// entire 1024×1280 PNG for each of 512 clipped `<use>` references. Only this
/// known generated atlas is normalized; the scientific model/counts are untouched.
fn atlas_tiles(svg: &str) -> Result<Cow<'_, str>> {
    if !svg.contains("id=\"minion-atlas\"") {
        return Ok(Cow::Borrowed(svg));
    }
    let document = roxmltree::Document::parse(svg).context("cannot parse MinION atlas SVG")?;
    let atlas = document
        .descendants()
        .find(|n| n.attribute("id") == Some("minion-atlas"))
        .context("missing MinION atlas")?;
    let uri = atlas
        .attribute("href")
        .context("missing MinION atlas PNG")?;
    let bytes = STANDARD.decode(
        uri.strip_prefix("data:image/png;base64,")
            .context("MinION atlas is not an embedded PNG")?,
    )?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.into_rgba8();
    let mut changes = vec![(atlas.range(), String::new())];
    for node in document
        .descendants()
        .filter(|n| n.attribute("href") == Some("#minion-atlas"))
    {
        let translation = node
            .attribute("transform")
            .and_then(|v| v.strip_prefix("translate("))
            .and_then(|v| v.strip_suffix(')'))
            .context("invalid generated atlas transform")?;
        let xy = translation
            .split_whitespace()
            .map(str::parse::<i32>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(xy.len() == 2, "invalid atlas translation");
        let clip_id = node
            .parent_element()
            .and_then(|n| n.attribute("clip-path"))
            .and_then(|s| s.strip_prefix("url(#"))
            .and_then(|s| s.strip_suffix(')'))
            .context("missing atlas panel clip")?;
        let clip = document
            .descendants()
            .find(|n| n.attribute("id") == Some(clip_id))
            .and_then(|n| n.children().find(|n| n.has_tag_name("rect")))
            .context("missing atlas clip rectangle")?;
        let number = |key| -> Result<i32> {
            Ok(clip
                .attribute(key)
                .context("missing atlas clip coordinate")?
                .parse()?)
        };
        let (x, y, w, h) = (
            number("x")?,
            number("y")?,
            number("width")?,
            number("height")?,
        );
        let (sx, sy) = (x - xy[0], y - xy[1]);
        ensure!(
            sx >= 0
                && sy >= 0
                && w > 0
                && h > 0
                && sx as u32 + w as u32 <= image.width()
                && sy as u32 + h as u32 <= image.height(),
            "atlas panel outside embedded PNG"
        );
        let mut tile = Vec::with_capacity(w as usize * h as usize * 4);
        for row in sy..sy + h {
            let start = (row as usize * image.width() as usize + sx as usize) * 4;
            tile.extend_from_slice(&image.as_raw()[start..start + w as usize * 4]);
        }
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, w as u32, h as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header()?.write_image_data(&tile)?;
        }
        changes.push((node.range(),format!("<image x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" href=\"data:image/png;base64,{}\"/>",STANDARD.encode(png))));
    }
    let mut result = svg.to_owned();
    changes.sort_unstable_by_key(|(r, _)| std::cmp::Reverse(r.start));
    for (range, replacement) in changes {
        result.replace_range(range, &replacement);
    }
    Ok(Cow::Owned(result))
}

/// PostScript Level 3, one lossless RGB image at 300 DPI. Paths/text have been
/// rasterized by resvg, not disguised as vectors. Flate + ASCIIHex keeps the
/// stream compact, portable, 7-bit clean and independent of external converters.
fn postscript(
    writer: &mut impl Write,
    rgb: &[u8],
    width: u32,
    height: u32,
    page_width: f32,
    page_height: f32,
) -> Result<()> {
    writeln!(
        writer,
        "%!PS-Adobe-3.0\n%%Creator: ONTRunQC\n%%Pages: 1\n%%BoundingBox: 0 0 {} {}\n%%HiResBoundingBox: 0 0 {page_width:.6} {page_height:.6}\n%%LanguageLevel: 3\n%%DocumentData: Clean7Bit\n%%EndComments\n%%Page: 1 1\n<< /PageSize [{page_width:.6} {page_height:.6}] >> setpagedevice\ngsave\n{page_width:.6} {page_height:.6} scale\n/DeviceRGB setcolorspace\n<< /ImageType 1 /Width {width} /Height {height} /BitsPerComponent 8\n/Decode [0 1 0 1 0 1] /ImageMatrix [{width} 0 0 -{height} 0 {height}]\n/DataSource currentfile /ASCIIHexDecode filter /FlateDecode filter >> image",
        page_width.ceil() as u32,
        page_height.ceil() as u32
    )?;
    let mut compressed = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    compressed.write_all(rgb)?;
    let bytes = compressed.finish()?;
    const HEX: &[u8] = b"0123456789abcdef";
    let mut line = [0; 64];
    for chunk in bytes.chunks(32) {
        for (i, &v) in chunk.iter().enumerate() {
            line[i * 2] = HEX[(v >> 4) as usize];
            line[i * 2 + 1] = HEX[(v & 15) as usize];
        }
        writer.write_all(&line[..chunk.len() * 2])?;
        writer.write_all(b"\n")?;
    }
    writer.write_all(b">\ngrestore\nshowpage\n%%EOF\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::io::{Cursor, Read};

    fn svg() -> String {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 1, 1);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[0, 255, 0])
                .unwrap();
        }
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="25.4mm" height="12.7mm" viewBox="0 0 100 50"><rect x="0" y="0" width="30" height="50" fill="#455998"/><rect x="30" y="0" width="30" height="50" fill="red" opacity="0.5"/><text x="61" y="42" font-family="Arial,sans-serif" font-size="9">Q ≥ 7</text><image x="65" y="0" width="20" height="20" href="data:image/png;base64,{}"/></svg>"##,
            STANDARD.encode(png)
        )
    }

    #[test]
    fn raster_formats_decode_with_colors_dimensions_transparency_and_embedded_png() {
        let renderer = Renderer::new();
        for (format, codec) in [
            (FigureFormat::Png, image::ImageFormat::Png),
            (FigureFormat::Jpeg, image::ImageFormat::Jpeg),
            (FigureFormat::Tiff, image::ImageFormat::Tiff),
            (FigureFormat::Bmp, image::ImageFormat::Bmp),
        ] {
            let mut output = Cursor::new(Vec::new());
            renderer.write(&svg(), format, &mut output).unwrap();
            let decoded = image::load_from_memory_with_format(output.get_ref(), codec)
                .unwrap()
                .to_rgb8();
            assert_eq!(decoded.dimensions(), (300, 150));
            for (x, y, color) in [
                (40, 75, [69, 89, 152]),
                (130, 75, [255, 127, 127]),
                (220, 30, [0, 255, 0]),
            ] {
                for (actual, expected) in decoded.get_pixel(x, y).0.into_iter().zip(color) {
                    assert!(
                        actual.abs_diff(expected) <= 5,
                        "{} at {x},{y}: {actual} vs {expected}",
                        format.extension()
                    );
                }
            }
            assert!(
                decoded
                    .enumerate_pixels()
                    .filter(|(x, y, _)| *x > 180 && *y > 95)
                    .any(|(_, _, p)| p.0.iter().all(|&v| v < 120)),
                "font must render"
            );
            match format {
                FigureFormat::Png => {
                    let reader = png::Decoder::new(Cursor::new(output.into_inner()))
                        .read_info()
                        .unwrap();
                    let density = reader.info().pixel_dims.unwrap();
                    assert_eq!(density.xppu, PIXELS_PER_METER);
                    assert_eq!(density.unit, png::Unit::Meter);
                }
                FigureFormat::Tiff => {
                    output.set_position(0);
                    let mut reader = tiff::decoder::Decoder::new(output).unwrap();
                    assert_eq!(
                        reader.get_tag_u32(tiff::tags::Tag::ResolutionUnit).unwrap(),
                        2
                    );
                    assert_eq!(
                        reader
                            .get_tag_u32_vec(tiff::tags::Tag::XResolution)
                            .unwrap(),
                        vec![300, 1]
                    );
                }
                FigureFormat::Jpeg => {
                    let data = output.get_ref();
                    let jfif = data.windows(5).position(|v| v == b"JFIF\0").unwrap();
                    assert_eq!(data[jfif + 7], 1); // Inches.
                    assert_eq!(
                        u16::from_be_bytes(data[jfif + 8..jfif + 10].try_into().unwrap()),
                        300
                    );
                    assert_eq!(
                        u16::from_be_bytes(data[jfif + 10..jfif + 12].try_into().unwrap()),
                        300
                    );
                }
                FigureFormat::Bmp => {
                    assert_eq!(&output.get_ref()[38..42], &PIXELS_PER_METER.to_le_bytes())
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn pdf_retains_vector_text_and_physical_page_dimensions() {
        let mut output = Cursor::new(Vec::new());
        Renderer::new()
            .write(&svg(), FigureFormat::Pdf, &mut output)
            .unwrap();
        let data = output.into_inner();
        assert!(data.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&data);
        assert!(text.contains("/MediaBox [0 0 72 36]"));
        assert!(text.contains("/Font"));
        assert!(text.contains("/Subtype /Image"));
        assert!(text.contains("/Type /Page"));
    }

    #[test]
    fn postscript_contains_a_real_lossless_rgb_page_not_another_file_format() {
        let renderer = Renderer::new();
        let mut ps = Cursor::new(Vec::new());
        renderer.write(&svg(), FigureFormat::Ps, &mut ps).unwrap();
        let ps = String::from_utf8(ps.into_inner()).unwrap();
        assert!(ps.starts_with("%!PS-Adobe-3.0"));
        assert!(ps.contains("%%HiResBoundingBox: 0 0 72.000000 36.000000"));
        assert!(ps.contains("/Width 300 /Height 150"));
        let encoded = ps
            .split_once("filter >> image\n")
            .unwrap()
            .1
            .split_once('>')
            .unwrap()
            .0;
        let hex: Vec<_> = encoded
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        let bytes: Vec<_> = hex
            .chunks_exact(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect();
        let mut rgb = Vec::new();
        flate2::read::ZlibDecoder::new(bytes.as_slice())
            .read_to_end(&mut rgb)
            .unwrap();
        let mut png = Cursor::new(Vec::new());
        renderer.write(&svg(), FigureFormat::Png, &mut png).unwrap();
        assert_eq!(
            rgb,
            image::load_from_memory(png.get_ref())
                .unwrap()
                .to_rgb8()
                .into_raw()
        );
    }

    #[test]
    fn invalid_svg_and_excessive_raster_sizes_fail_actionably() {
        let renderer = Renderer::new();
        assert!(
            renderer
                .write("<svg", FigureFormat::Png, &mut Cursor::new(Vec::new()))
                .is_err()
        );
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10000mm" height="10000mm"/>"#;
        let error = renderer
            .write(svg, FigureFormat::Png, &mut Cursor::new(Vec::new()))
            .unwrap_err();
        assert!(error.to_string().contains("maximum"));
    }

    #[test]
    fn atlas_normalization_preserves_rgba_cells_and_panel_positions() {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 2, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[255, 0, 0, 255, 0, 255, 0, 128])
                .unwrap();
        }
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="25.4mm" height="12.7mm" viewBox="0 0 4 2"><defs><image id="minion-atlas" width="2" height="1" href="data:image/png;base64,{}"/><clipPath id="a"><rect x="0" y="0" width="1" height="1"/></clipPath><clipPath id="b"><rect x="3" y="0" width="1" height="1"/></clipPath></defs><g clip-path="url(#a)"><use href="#minion-atlas" transform="translate(0 0)"/></g><g clip-path="url(#b)"><use href="#minion-atlas" transform="translate(2 0)"/></g></svg>"##,
            STANDARD.encode(png)
        );
        let normalized = atlas_tiles(&svg).unwrap();
        let tree = roxmltree::Document::parse(&normalized).unwrap();
        let tiles: Vec<_> = tree
            .descendants()
            .filter(|n| n.has_tag_name("image"))
            .collect();
        assert_eq!(tiles.len(), 2);
        for (node, (x, rgba)) in tiles
            .iter()
            .zip([(0, [255, 0, 0, 255]), (3, [0, 255, 0, 128])])
        {
            assert_eq!(node.attribute("x").unwrap(), x.to_string());
            let bytes = STANDARD
                .decode(node.attribute("href").unwrap().split_once(',').unwrap().1)
                .unwrap();
            let tile = image::load_from_memory(&bytes).unwrap().into_rgba8();
            assert_eq!(tile.dimensions(), (1, 1));
            assert_eq!(tile.get_pixel(0, 0).0, rgba);
        }
        let mut output = Cursor::new(Vec::new());
        Renderer::new()
            .write(&svg, FigureFormat::Png, &mut output)
            .unwrap();
        let decoded = image::load_from_memory(output.get_ref())
            .unwrap()
            .into_rgb8();
        assert_eq!(decoded.get_pixel(30, 30).0, [255, 0, 0]);
        assert_eq!(decoded.get_pixel(250, 30).0, [127, 255, 127]);
        assert_eq!(decoded.get_pixel(150, 30).0, [255, 255, 255]);
    }

    #[test]
    fn stable_raster_payloads_and_all_conversion_failures() {
        struct Failed;
        impl Write for Failed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("test write failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl Seek for Failed {
            fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
                Ok(0)
            }
        }
        let renderer = Renderer::new();
        for format in [
            FigureFormat::Png,
            FigureFormat::Pdf,
            FigureFormat::Ps,
            FigureFormat::Jpeg,
            FigureFormat::Tiff,
            FigureFormat::Bmp,
        ] {
            let mut a = Cursor::new(Vec::new());
            let mut b = Cursor::new(Vec::new());
            renderer.write(&svg(), format, &mut a).unwrap();
            renderer.write(&svg(), format, &mut b).unwrap();
            // svg2pdf may order equivalent PDF resources differently. Names,
            // pages and scientific content are stable, but PDF byte identity
            // is not its contract. Other encoders have no variable metadata.
            if !matches!(format, FigureFormat::Pdf) {
                assert!(
                    a.get_ref() == b.get_ref(),
                    "{} encoding changed",
                    format.extension()
                );
            } else {
                assert!(a.get_ref().starts_with(b"%PDF-") && b.get_ref().starts_with(b"%PDF-"));
            }
            assert!(renderer.write(&svg(), format, &mut Failed).is_err());
        }
    }

    #[test]
    #[ignore = "requires Ghostscript and Poppler; CI executes this explicitly"]
    fn external_pdf_postscript_decoders() {
        use std::{fs, process::Command};
        let directory =
            std::env::temp_dir().join(format!("ontrunqc-static-decoders-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let renderer = Renderer::new();
        let gs = std::env::var("ONTRUNQC_GHOSTSCRIPT")
            .or_else(|_| std::env::var("POREGAUGE_GHOSTSCRIPT"))
            .unwrap_or_else(|_| "gs".into());
        for format in [FigureFormat::Pdf, FigureFormat::Ps] {
            let source = directory.join(format!("figure.{}", format.extension()));
            renderer
                .write(&svg(), format, &mut fs::File::create(&source).unwrap())
                .unwrap();
            let decoded = directory.join(format!("{}.png", format.extension()));
            let command = if matches!(format, FigureFormat::Pdf) {
                let text = Command::new("pdftotext")
                    .arg(&source)
                    .arg("-")
                    .output()
                    .unwrap();
                assert!(text.status.success());
                // PDF extractors may infer/remove visual word spacing; the
                // actual selectable symbols and threshold must survive.
                assert_eq!(
                    String::from_utf8_lossy(&text.stdout)
                        .split_whitespace()
                        .collect::<String>(),
                    "Q≥7"
                );
                Command::new("pdftoppm")
                    .args(["-png", "-singlefile", "-r", "300"])
                    .arg(&source)
                    .arg(decoded.with_extension(""))
                    .output()
                    .unwrap()
            } else {
                Command::new(&gs)
                    .args([
                        "-q",
                        "-dSAFER",
                        "-dBATCH",
                        "-dNOPAUSE",
                        "-sDEVICE=png16m",
                        "-r300",
                    ])
                    .arg(format!("-sOutputFile={}", decoded.display()))
                    .arg(&source)
                    .output()
                    .unwrap()
            };
            assert!(
                command.status.success(),
                "{}",
                String::from_utf8_lossy(&command.stderr)
            );
            let image = image::open(decoded).unwrap().into_rgb8();
            assert_eq!(image.dimensions(), (300, 150));
            for (x, y, expected) in [
                (40, 75, [69, 89, 152]),
                (130, 75, [255, 127, 127]),
                (220, 30, [0, 255, 0]),
            ] {
                assert!(
                    image
                        .get_pixel(x, y)
                        .0
                        .into_iter()
                        .zip(expected)
                        .all(|(a, b)| a.abs_diff(b) <= 2)
                );
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
