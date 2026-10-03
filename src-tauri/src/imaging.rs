use crate::error::AppError;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{ColorType, DynamicImage, ImageDecoder, ImageReader, Rgb, RgbImage};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const JPEG_QUALITY: u8 = 85;
const THUMB_SIZE: u32 = 512;
const THUMB_QUALITY: u8 = 80;

pub struct Decoded {
    img: DynamicImage,
    orientation: Orientation,
}

fn turns_sideways(o: Orientation) -> bool {
    matches!(
        o,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

pub fn file_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().to_string(),
        None => path.to_string_lossy().to_string(),
    }
}

// we can't open iphone HEIC files, so catch them and give a clear error
fn check_not_heic(path: &Path, name: &str) -> Result<(), AppError> {
    let mut file = File::open(path).map_err(|e| AppError::read_failed(e, name))?;
    let mut start = [0u8; 12];
    let count = file.read(&mut start).map_err(|e| AppError::read_failed(e, name))?;

    let mut heic_by_content = false;
    if count == 12 && &start[4..8] == b"ftyp" {
        heic_by_content = matches!(
            &start[8..12],
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"hevm" | b"hevs" | b"mif1" | b"msf1"
        );
    }

    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let heic_by_name = extension == "heic" || extension == "heif";

    if heic_by_content || heic_by_name {
        return Err(AppError::new("heic", Some(name), "HEIC is not supported"));
    }
    Ok(())
}

// reads the whole photo into memory
pub fn decode(path: &Path) -> Result<Decoded, AppError> {
    let name = file_name(path);
    check_not_heic(path, &name)?;

    let reader = ImageReader::open(path).map_err(|e| AppError::read_failed(e, &name))?;
    let reader = reader
        .with_guessed_format()
        .map_err(|e| AppError::read_failed(e, &name))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| AppError::from_image(e, &name))?;

    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let img = DynamicImage::from_decoder(decoder).map_err(|e| AppError::from_image(e, &name))?;

    Ok(Decoded { img, orientation })
}

impl Decoded {
    // true if the final image has width and height swapped compared to the
    // stored pixels.
    fn is_swapped(&self, quarters: u8) -> bool {
        let exif_swaps = turns_sideways(self.orientation);
        let user_swaps = quarters % 2 == 1;
        exif_swaps != user_swaps
    }

    pub fn final_size(&self, quarters: u8) -> (u32, u32) {
        let width = self.img.width();
        let height = self.img.height();
        if self.is_swapped(quarters) {
            (height, width)
        } else {
            (width, height)
        }
    }

    // shrinks to target, then fixes the orientation
    // and applies the user's rotation.
    pub fn render(self, quarters: u8, target: Option<(u32, u32)>) -> DynamicImage {
        let swapped = self.is_swapped(quarters);
        let orientation = self.orientation;
        let mut img = self.img;

        if let Some((target_w, target_h)) = target {
            let (w, h) = if swapped { (target_h, target_w) } else { (target_w, target_h) };
            if w < img.width() && h < img.height() {
                img = img.resize_exact(w.max(1), h.max(1), FilterType::CatmullRom);
            }
        }

        img.apply_orientation(orientation);

        match quarters % 4 {
            1 => img.rotate90(),
            2 => img.rotate180(),
            3 => img.rotate270(),
            _ => img,
        }
    }
}

// makes a jpeg. transparent parts become white, gray photos stay gray.
// returns the bytes and whether it's gray
pub fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<(Vec<u8>, bool), image::ImageError> {
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);

    let is_gray = matches!(img.color(), ColorType::L8 | ColorType::L16);
    if is_gray {
        encoder.encode_image(&img.to_luma8())?;
    } else if img.color().has_alpha() {
        encoder.encode_image(&put_on_white(img))?;
    } else {
        encoder.encode_image(&img.to_rgb8())?;
    }
    Ok((bytes, is_gray))
}

fn put_on_white(img: &DynamicImage) -> RgbImage {
    let rgba = img.to_rgba8();
    let mut result = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as u32;
        let mix = |c: u8| ((c as u32 * alpha + 255 * (255 - alpha)) / 255) as u8;
        result.put_pixel(x, y, Rgb([mix(pixel[0]), mix(pixel[1]), mix(pixel[2])]));
    }
    result
}

// small jpeg for the screen. also returns the full size of the photo
pub fn thumbnail(path: &Path) -> Result<(Vec<u8>, u32, u32), AppError> {
    let name = file_name(path);
    let decoded = decode(path)?;
    let (width, height) = decoded.final_size(0);

    let longest = width.max(height);
    let mut target = None;
    if longest > THUMB_SIZE {
        let scale = THUMB_SIZE as f64 / longest as f64;
        let thumb_w = ((width as f64 * scale).round() as u32).max(1);
        let thumb_h = ((height as f64 * scale).round() as u32).max(1);
        target = Some((thumb_w, thumb_h));
    }

    let small = decoded.render(0, target);
    let (bytes, _) = encode_jpeg(&small, THUMB_QUALITY).map_err(|e| AppError::from_image(e, &name))?;
    Ok((bytes, width, height))
}
