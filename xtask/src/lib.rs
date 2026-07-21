use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

const PNG_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256, 512, 1024];
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
const ICNS_SIZES: &[u32] = &[16, 32, 64, 128, 256, 512, 1024];

pub fn icons(source: impl AsRef<Path>, output: impl AsRef<Path>)
    -> Result<(), Box<dyn std::error::Error>>
{
    let source = source.as_ref();
    let output = output.as_ref();
    let svg = fs::read(source)?;
    let tree = resvg::usvg::Tree::from_data(
        &svg,
        &resvg::usvg::Options::default(),
    )?;
    fs::create_dir_all(output)?;

    let mut rgba = Vec::new();
    for size in PNG_SIZES {
        let pixels = rasterize(&tree, *size)?;
        write_png(&output.join(format!("strop-{size}.png")), *size, &pixels)?;
        let icon_dir = output.join(format!(
            "hicolor/{size}x{size}/apps"
        ));
        fs::create_dir_all(&icon_dir)?;
        write_png(
            &icon_dir.join("cc.pimenov.strop.png"),
            *size,
            &pixels,
        )?;
        rgba.push((*size, pixels));
    }

    let mut ico = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let pixels = pixels(&rgba, *size);
        let image = ico::IconImage::from_rgba_data(*size, *size, pixels.clone());
        ico.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    ico.write(BufWriter::new(File::create(output.join("strop.ico"))?))?;

    let mut family = icns::IconFamily::new();
    for size in ICNS_SIZES {
        let image = icns::Image::from_data(
            icns::PixelFormat::RGBA,
            *size,
            *size,
            pixels(&rgba, *size).clone(),
        )?;
        let icon_type = icns::IconType::from_pixel_size(*size, *size)
            .ok_or("unsupported icns size")?;
        family.add_icon_with_type(&image, icon_type)?;
    }
    family.write(BufWriter::new(File::create(output.join("strop.icns"))?))?;

    let scalable = output.join("hicolor/scalable/apps");
    fs::create_dir_all(&scalable)?;
    fs::copy(source, scalable.join("cc.pimenov.strop.svg"))?;
    Ok(())
}

fn rasterize(
    tree: &resvg::usvg::Tree,
    size: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or("invalid pixmap size")?;
    let svg_size = tree.size();
    let transform = resvg::tiny_skia::Transform::from_scale(
        size as f32 / svg_size.width(),
        size as f32 / svg_size.height(),
    );
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap.data().to_vec())
}

fn write_png(path: &Path, size: u32, pixels: &[u8])
    -> Result<(), Box<dyn std::error::Error>>
{
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn pixels(images: &[(u32, Vec<u8>)], size: u32) -> &Vec<u8> {
    &images.iter().find(|(candidate, _)| *candidate == size).unwrap().1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generated_icons_are_parseable() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_nanos();
        let output = std::env::temp_dir().join(format!(
            "strop-icons-{}-{unique}", std::process::id()
        ));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        icons(root.join("assets/icon/strop-mark.svg"), &output).unwrap();

        for size in PNG_SIZES {
            let decoder = png::Decoder::new(BufReader::new(
                File::open(output.join(format!("strop-{size}.png"))).unwrap(),
            ));
            let reader = decoder.read_info().unwrap();
            assert_eq!((reader.info().width, reader.info().height), (*size, *size));
        }
        let ico = ico::IconDir::read(BufReader::new(
            File::open(output.join("strop.ico")).unwrap(),
        )).unwrap();
        assert_eq!(ico.entries().len(), ICO_SIZES.len());
        for entry in ico.entries() { entry.decode().unwrap(); }
        let icns = icns::IconFamily::read(BufReader::new(
            File::open(output.join("strop.icns")).unwrap(),
        )).unwrap();
        for size in ICNS_SIZES {
            let kind = icns::IconType::from_pixel_size(*size, *size).unwrap();
            icns.get_icon_with_type(kind).unwrap();
        }
        fs::remove_dir_all(output).unwrap();
    }
}
