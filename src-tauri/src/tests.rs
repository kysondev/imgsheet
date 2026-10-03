use crate::commands::import_photo_sync;
use crate::imaging;
use crate::pdf::{self, fit, Margin, PageSize, PageSpec};
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, DynamicImage, Rgb, RgbImage, Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const RED: [u8; 3] = [220, 30, 30];
const GREEN: [u8; 3] = [30, 180, 30];
const BLUE: [u8; 3] = [30, 60, 220];
const YELLOW: [u8; 3] = [240, 220, 20];

struct TempDir {
    folder: PathBuf,
}

impl TempDir {
    fn new() -> TempDir {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let number = COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("imgsheet-test-{}-{}", std::process::id(), number);
        let folder = std::env::temp_dir().join(name);
        fs::create_dir_all(&folder).unwrap();
        TempDir { folder }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.folder.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
    }
}

fn test_picture(width: u32, height: u32) -> RgbImage {
    let square = width.min(height) / 8;
    let mut img = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let left = x < square;
            let right = x >= width - square;
            let top = y < square;
            let bottom = y >= height - square;

            let color = if left && top {
                RED
            } else if right && top {
                GREEN
            } else if left && bottom {
                BLUE
            } else if right && bottom {
                YELLOW
            } else {
                [235, 232, 225]
            };
            img.put_pixel(x, y, Rgb(color));
        }
    }
    img
}

fn transpose(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut result = RgbImage::new(h, w);
    for y in 0..h {
        for x in 0..w {
            result.put_pixel(y, x, *img.get_pixel(x, y));
        }
    }
    result
}

fn transverse(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut result = RgbImage::new(h, w);
    for y in 0..h {
        for x in 0..w {
            result.put_pixel(h - 1 - y, w - 1 - x, *img.get_pixel(x, y));
        }
    }
    result
}

fn stored_pixels(picture: &RgbImage, exif: u8) -> RgbImage {
    match exif {
        1 => picture.clone(),
        2 => imageops::flip_horizontal(picture),
        3 => imageops::rotate180(picture),
        4 => imageops::flip_vertical(picture),
        5 => transpose(picture),
        6 => imageops::rotate270(picture),
        7 => transverse(picture),
        8 => imageops::rotate90(picture),
        _ => panic!("exif must be 1 to 8"),
    }
}

fn save_photo(dir: &TempDir, name: &str, picture: &RgbImage, exif: u8) -> PathBuf {
    let stored = stored_pixels(picture, exif);
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 95)
        .encode_image(&stored)
        .unwrap();

    let mut exif_block = vec![0xFF, 0xE1, 0x00, 34];
    exif_block.extend_from_slice(b"Exif\0\0");
    exif_block.extend_from_slice(&[b'M', b'M', 0, 42, 0, 0, 0, 8]);
    exif_block.extend_from_slice(&[0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, exif, 0, 0, 0, 0, 0, 0]);

    let mut file_bytes = jpeg[..2].to_vec();
    file_bytes.extend(exif_block);
    file_bytes.extend_from_slice(&jpeg[2..]);

    let path = dir.file(name);
    fs::write(&path, file_bytes).unwrap();
    path
}

fn colors_match(a: [u8; 3], b: [u8; 3]) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() < 40)
}

fn check_corners(img: &DynamicImage, expected: [[u8; 3]; 4]) {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let dx = (w / 25).max(1);
    let dy = (h / 25).max(1);
    let spots = [
        (dx, dy),
        (w - 1 - dx, dy),
        (dx, h - 1 - dy),
        (w - 1 - dx, h - 1 - dy),
    ];
    for i in 0..4 {
        let (x, y) = spots[i];
        let got = rgb.get_pixel(x, y).0;
        assert!(
            colors_match(got, expected[i]),
            "corner {} was {:?}, expected {:?}",
            i,
            got,
            expected[i]
        );
    }
}

fn page(path: &Path, rotation: i32) -> PageSpec {
    PageSpec {
        path: path.to_string_lossy().to_string(),
        rotation,
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}


#[test]
fn portrait_photo_on_letter() {
    let p = fit(PageSize::Letter, Margin::Small, 3024.0, 4032.0);
    assert!(close(p.h, 756.0));
    assert!(close(p.w, 567.0));
    assert!(close(p.x, 22.5));
    assert!(close(p.y, 18.0));
}

#[test]
fn landscape_photo_on_letter() {
    let p = fit(PageSize::Letter, Margin::Small, 4032.0, 3024.0);
    assert!(close(p.w, 576.0));
    assert!(close(p.h, 432.0));
    assert!(close(p.x, 18.0));
    assert!(close(p.y, 180.0));
}

#[test]
fn square_photo_on_a4_no_margin() {
    let (page_w, page_h) = PageSize::A4.points();
    let p = fit(PageSize::A4, Margin::None, 1000.0, 1000.0);
    assert!(close(p.w, page_w));
    assert!(close(p.h, page_w));
    assert!(close(p.x, 0.0));
    assert!(close(p.y, (page_h - page_w) / 2.0));
}

#[test]
fn photo_always_stays_inside_the_margins_and_keeps_its_shape() {
    let sizes = [PageSize::Letter, PageSize::A4];
    let margins = [Margin::None, Margin::Small, Margin::Medium];
    let photos = [
        (4032.0, 3024.0),
        (3024.0, 4032.0),
        (4032.0, 2268.0),
        (1000.0, 4000.0),
        (4000.0, 1000.0),
        (50.0, 50.0),
        (1.0, 5000.0),
    ];

    for size in sizes {
        for margin in margins {
            for (photo_w, photo_h) in photos {
                let p = fit(size, margin, photo_w, photo_h);
                let (page_w, page_h) = size.points();
                let m = margin.points();
                let tiny = 0.001;

                assert!(p.x >= m - tiny && p.y >= m - tiny);
                assert!(p.x + p.w <= page_w - m + tiny);
                assert!(p.y + p.h <= page_h - m + tiny);

                let photo_shape = photo_w / photo_h;
                let placed_shape = p.w / p.h;
                assert!((placed_shape - photo_shape).abs() / photo_shape < 0.0001);
            }
        }
    }
}


#[test]
fn all_8_exif_orientations_come_out_upright() {
    let dir = TempDir::new();
    let picture = test_picture(200, 150);

    for exif in 1..=8u8 {
        let path = save_photo(&dir, &format!("exif{}.jpg", exif), &picture, exif);
        let decoded = imaging::decode(&path).unwrap();
        assert_eq!(decoded.final_size(0), (200, 150), "exif {}", exif);

        let img = decoded.render(0, None);
        assert_eq!((img.width(), img.height()), (200, 150), "exif {}", exif);
        check_corners(&img, [RED, GREEN, BLUE, YELLOW]);
    }
}

#[test]
fn user_rotation_turns_clockwise_after_exif() {
    let dir = TempDir::new();
    let path = save_photo(&dir, "a.jpg", &test_picture(200, 150), 6);

    let cases = [
        (0, (200, 150), [RED, GREEN, BLUE, YELLOW]),
        (1, (150, 200), [BLUE, RED, YELLOW, GREEN]),
        (2, (200, 150), [YELLOW, BLUE, GREEN, RED]),
        (3, (150, 200), [GREEN, YELLOW, RED, BLUE]),
    ];
    for (quarters, size, corners) in cases {
        let decoded = imaging::decode(&path).unwrap();
        assert_eq!(decoded.final_size(quarters), size);
        let img = decoded.render(quarters, None);
        assert_eq!((img.width(), img.height()), size);
        check_corners(&img, corners);
    }
}

#[test]
fn every_rotation_number_reaches_the_pdf_page() {
    let dir = TempDir::new();
    let path = save_photo(&dir, "a.jpg", &test_picture(400, 300), 6);

    let cases = [
        (0, [RED, GREEN, BLUE, YELLOW]),
        (90, [BLUE, RED, YELLOW, GREEN]),
        (180, [YELLOW, BLUE, GREEN, RED]),
        (270, [GREEN, YELLOW, RED, BLUE]),
        (-90, [GREEN, YELLOW, RED, BLUE]),
        (-270, [BLUE, RED, YELLOW, GREEN]),
        (360, [RED, GREEN, BLUE, YELLOW]),
        (450, [BLUE, RED, YELLOW, GREEN]),
    ];
    for (rotation, corners) in cases {
        let result = pdf::render_page(&page(&path, rotation), PageSize::Letter, Margin::Small).unwrap();
        let img = image::load_from_memory(&result.jpeg).unwrap();
        check_corners(&img, corners);

        let is_tall = result.place.h > result.place.w;
        let should_be_tall = rotation.rem_euclid(180) == 90;
        assert_eq!(is_tall, should_be_tall, "rotation {}", rotation);
    }
}

#[test]
fn shrinking_works_with_exif_and_rotation_together() {
    let dir = TempDir::new();
    let picture = test_picture(400, 300);

    for exif in [1u8, 6, 5, 8] {
        let path = save_photo(&dir, &format!("s{}.jpg", exif), &picture, exif);
        let decoded = imaging::decode(&path).unwrap();
        assert_eq!(decoded.final_size(1), (300, 400));

        let img = decoded.render(1, Some((150, 200)));
        assert_eq!((img.width(), img.height()), (150, 200), "exif {}", exif);
        check_corners(&img, [BLUE, RED, YELLOW, GREEN]);
    }
}

#[test]
fn small_pictures_are_not_made_bigger() {
    let dir = TempDir::new();
    let path = save_photo(&dir, "tiny.jpg", &test_picture(100, 100), 1);
    let result = pdf::render_page(&page(&path, 0), PageSize::Letter, Margin::Small).unwrap();
    assert_eq!((result.px_w, result.px_h), (100, 100));
}

#[test]
fn big_pictures_are_shrunk_to_about_200_dpi() {
    let dir = TempDir::new();
    let path = save_photo(&dir, "big.jpg", &test_picture(3000, 4000), 1);
    let result = pdf::render_page(&page(&path, 0), PageSize::Letter, Margin::Small).unwrap();
    assert!(result.px_w >= 1570 && result.px_w <= 1580, "width {}", result.px_w);
    assert!(result.px_h >= 2095 && result.px_h <= 2105, "height {}", result.px_h);
}

#[test]
fn format_comes_from_the_contents_not_the_name() {
    let dir = TempDir::new();
    let path = dir.file("really_a_png.jpg");
    DynamicImage::ImageRgb8(test_picture(64, 48))
        .save_with_format(&path, image::ImageFormat::Png)
        .unwrap();
    let decoded = imaging::decode(&path).unwrap();
    assert_eq!(decoded.final_size(0), (64, 48));
}

#[test]
fn see_through_png_parts_become_white() {
    let dir = TempDir::new();
    let path = dir.file("alpha.png");
    let mut img = RgbaImage::from_pixel(60, 60, Rgba([10, 10, 10, 255]));
    img.put_pixel(30, 30, Rgba([255, 0, 0, 0]));
    DynamicImage::ImageRgba8(img).save(&path).unwrap();

    let img = imaging::decode(&path).unwrap().render(0, None);
    let (jpeg, is_gray) = imaging::encode_jpeg(&img, 95).unwrap();
    assert!(!is_gray);

    let back = image::load_from_memory(&jpeg).unwrap().to_rgb8();
    let pixel = back.get_pixel(30, 30).0;
    assert!(pixel.iter().all(|c| *c > 200), "pixel was {:?}", pixel);
}

#[test]
fn gray_pictures_stay_gray() {
    let dir = TempDir::new();
    let path = dir.file("gray.png");
    DynamicImage::ImageRgb8(test_picture(80, 60)).to_luma8().save(&path).unwrap();
    let result = pdf::render_page(&page(&path, 0), PageSize::A4, Margin::None).unwrap();
    assert!(result.gray);
}

#[test]
fn heic_gets_its_own_error() {
    let dir = TempDir::new();

    let mut bytes = vec![0, 0, 0, 24];
    bytes.extend_from_slice(b"ftypheic");
    bytes.extend_from_slice(&[0; 32]);

    let by_name = dir.file("IMG_0001.HEIC");
    fs::write(&by_name, &bytes).unwrap();
    let error = imaging::decode(&by_name).err().unwrap();
    assert_eq!(error.code, "heic");
    assert_eq!(error.file.as_deref(), Some("IMG_0001.HEIC"));

    let renamed = dir.file("renamed.jpg");
    fs::write(&renamed, &bytes).unwrap();
    assert_eq!(imaging::decode(&renamed).err().unwrap().code, "heic");
}

#[test]
fn junk_and_empty_files_give_an_error() {
    let dir = TempDir::new();

    let junk = dir.file("notes.jpg");
    fs::write(&junk, b"this is not an image at all").unwrap();
    let error = imaging::decode(&junk).err().unwrap();
    assert!(error.code == "unsupported" || error.code == "corrupt", "code was {}", error.code);

    let empty = dir.file("empty.png");
    fs::write(&empty, b"").unwrap();
    assert!(imaging::decode(&empty).is_err());
}

#[test]
fn missing_file_is_unreadable() {
    let dir = TempDir::new();
    let error = imaging::decode(&dir.file("gone.jpg")).err().unwrap();
    assert_eq!(error.code, "unreadable");
    assert_eq!(error.file.as_deref(), Some("gone.jpg"));
}


#[test]
fn thumbnail_is_small_upright_and_reports_the_full_size() {
    let dir = TempDir::new();
    let path = save_photo(&dir, "phone.jpg", &test_picture(1500, 2000), 6);

    let info = import_photo_sync(&path).unwrap();
    assert_eq!((info.width, info.height), (1500, 2000));
    assert_eq!(info.name, "phone.jpg");

    let prefix = "data:image/jpeg;base64,";
    assert!(info.thumb.starts_with(prefix));
    let encoded = &info.thumb[prefix.len()..];

    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let thumb = image::load_from_memory(&STANDARD.decode(encoded).unwrap()).unwrap();
    assert_eq!((thumb.width(), thumb.height()), (384, 512));
    check_corners(&thumb, [RED, GREEN, BLUE, YELLOW]);
}


#[test]
fn builds_a_pdf_with_every_page() {
    let dir = TempDir::new();
    let a = save_photo(&dir, "a.jpg", &test_picture(300, 400), 1);
    let b = save_photo(&dir, "b.jpg", &test_picture(400, 300), 6);
    let c = dir.file("c.png");
    DynamicImage::ImageRgb8(test_picture(200, 200)).save(&c).unwrap();

    let pages = [page(&a, 0), page(&b, 90), page(&c, -90)];
    let mut updates = Vec::new();
    let bytes = pdf::build(&pages, PageSize::Letter, Margin::Small, |done, total| {
        updates.push((done, total));
    })
    .unwrap();

    assert_eq!(updates, vec![(1, 3), (2, 3), (3, 3)]);
    assert!(bytes.starts_with(b"%PDF-"));

    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Count 3"));
    assert_eq!(text.matches("/DCTDecode").count(), 3);
    assert_eq!(text.matches("/MediaBox [0 0 612 792]").count(), 3);
    assert!(text.trim_end().ends_with("%%EOF"));
}

#[test]
fn a4_page_size_is_exact() {
    let dir = TempDir::new();
    let a = save_photo(&dir, "a.jpg", &test_picture(300, 400), 1);
    let bytes = pdf::build(&[page(&a, 0)], PageSize::A4, Margin::None, |_, _| {}).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("/MediaBox [0 0 595.276 841.89]"));
}

#[test]
fn export_stops_and_names_the_bad_file() {
    let dir = TempDir::new();
    let good = save_photo(&dir, "good.jpg", &test_picture(100, 100), 1);
    let missing = dir.file("deleted.jpg");

    let pages = [page(&good, 0), page(&missing, 0)];
    let error = pdf::build(&pages, PageSize::Letter, Margin::Small, |_, _| {}).err().unwrap();
    assert_eq!(error.code, "unreadable");
    assert_eq!(error.file.as_deref(), Some("deleted.jpg"));
}

#[test]
fn rotation_must_be_a_multiple_of_90() {
    let dir = TempDir::new();
    let good = save_photo(&dir, "good.jpg", &test_picture(100, 100), 1);
    let error = pdf::build(&[page(&good, 45)], PageSize::Letter, Margin::Small, |_, _| {})
        .err()
        .unwrap();
    assert_eq!(error.code, "invalid");
}

#[test]
fn saving_replaces_the_old_file_and_leaves_no_temp_file() {
    let dir = TempDir::new();
    let out = dir.file("Homework.pdf");
    fs::write(&out, b"old stuff").unwrap();

    pdf::write_atomic(&out, b"%PDF-new").unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"%PDF-new");
    let files_in_folder = fs::read_dir(&dir.folder).unwrap().count();
    assert_eq!(files_in_folder, 1);
}

#[test]
fn saving_into_a_missing_folder_fails() {
    let dir = TempDir::new();
    let error = pdf::write_atomic(&dir.file("nope/Homework.pdf"), b"x").err().unwrap();
    assert_eq!(error.code, "write_failed");
}
#[test]
#[ignore]
fn bench_export() {
    let (Ok(dir), Ok(out)) = (std::env::var("IMGSHEET_BENCH_DIR"), std::env::var("IMGSHEET_BENCH_OUT")) else {
        return;
    };

    let list = fs::read_to_string(Path::new(&dir).join("pages.tsv")).unwrap();
    let mut pages = Vec::new();
    for line in list.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        pages.push(PageSpec {
            path: parts[0].to_string(),
            rotation: parts[1].parse().unwrap(),
        });
    }

    let start = std::time::Instant::now();
    let bytes = pdf::build(&pages, PageSize::Letter, Margin::Small, |_, _| {}).unwrap();
    println!(
        "{} pages, {:.1} MB, {:.2}s",
        pages.len(),
        bytes.len() as f64 / 1e6,
        start.elapsed().as_secs_f64()
    );
    fs::write(out, bytes).unwrap();
}
