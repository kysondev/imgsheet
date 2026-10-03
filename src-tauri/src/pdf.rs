use crate::error::AppError;
use crate::imaging;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};
use serde::Deserialize;
use std::fs;
use std::path::Path;

// pictures get shrunk to about this many pixels per inch of page
const DPI: f32 = 200.0;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageSize {
    Letter,
    A4,
}

impl PageSize {
    // width and height in pdf points (72 points = 1 inch)
    pub fn points(self) -> (f32, f32) {
        match self {
            PageSize::Letter => (612.0, 792.0),
            PageSize::A4 => (595.276, 841.890),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Margin {
    None,
    Small,
    Medium,
}

impl Margin {
    pub fn points(self) -> f32 {
        match self {
            Margin::None => 0.0,
            Margin::Small => 18.0,  // quarter inch
            Margin::Medium => 36.0, // half inch
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub fn fit(size: PageSize, margin: Margin, photo_w: f32, photo_h: f32) -> Placement {
    let (page_w, page_h) = size.points();
    let m = margin.points();
    let area_w = page_w - 2.0 * m;
    let area_h = page_h - 2.0 * m;

    // scale until it hits either the side or the top/bottom
    let scale = (area_w / photo_w).min(area_h / photo_h);
    let w = photo_w * scale;
    let h = photo_h * scale;

    Placement {
        x: m + (area_w - w) / 2.0,
        y: m + (area_h - h) / 2.0,
        w,
        h,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSpec {
    pub path: String,
    pub rotation: i32,
}

pub struct EncodedPage {
    pub jpeg: Vec<u8>,
    pub px_w: u32,
    pub px_h: u32,
    pub gray: bool,
    pub place: Placement,
}

fn quarter_turns(rotation: i32, file: &str) -> Result<u8, AppError> {
    if rotation % 90 != 0 {
        let detail = format!("rotation {} is not a multiple of 90", rotation);
        return Err(AppError::new("invalid", Some(file), detail));
    }
    let degrees = rotation.rem_euclid(360);
    Ok((degrees / 90) as u8)
}

// opens one photo, shrinks it to fit the page and turns it into a jpeg
pub fn render_page(spec: &PageSpec, size: PageSize, margin: Margin) -> Result<EncodedPage, AppError> {
    let path = Path::new(&spec.path);
    let name = imaging::file_name(path);
    let quarters = quarter_turns(spec.rotation, &name)?;

    let decoded = imaging::decode(path)?;
    let (photo_w, photo_h) = decoded.final_size(quarters);
    let place = fit(size, margin, photo_w as f32, photo_h as f32);

    // how many pixels we actually need for the size it will be on the page
    let want_w = (place.w / 72.0 * DPI).ceil() as u32;
    let want_h = (place.h / 72.0 * DPI).ceil() as u32;

    let img = decoded.render(quarters, Some((want_w, want_h)));
    let (jpeg, gray) = imaging::encode_jpeg(&img, imaging::JPEG_QUALITY)
        .map_err(|e| AppError::from_image(e, &name))?;

    Ok(EncodedPage {
        jpeg,
        px_w: img.width(),
        px_h: img.height(),
        gray,
        place,
    })
}

// puts one page into the pdf. each page needs 3 objects: the page,
// its content (where to draw), and the image itself
fn add_page(pdf: &mut Pdf, tree_id: Ref, ids: [Ref; 3], size: PageSize, page: &EncodedPage) {
    let [page_id, content_id, image_id] = ids;
    let (page_w, page_h) = size.points();
    let image_name = Name(b"Im0");

    let mut pdf_page = pdf.page(page_id);
    pdf_page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
    pdf_page.parent(tree_id);
    pdf_page.contents(content_id);
    pdf_page.resources().x_objects().pair(image_name, image_id);
    pdf_page.finish();

    // draw the image: the 6 numbers are width, 0, 0, height, x, y
    let p = page.place;
    let mut content = Content::new();
    content.save_state();
    content.transform([p.w, 0.0, 0.0, p.h, p.x, p.y]);
    content.x_object(image_name);
    content.restore_state();
    pdf.stream(content_id, &content.finish());

    let mut image = pdf.image_xobject(image_id, &page.jpeg);
    image.filter(Filter::DctDecode); // means "this is a jpeg"
    image.width(page.px_w as i32);
    image.height(page.px_h as i32);
    image.bits_per_component(8);
    if page.gray {
        image.color_space().device_gray();
    } else {
        image.color_space().device_rgb();
    }
    image.finish();
}

pub fn build(
    pages: &[PageSpec],
    size: PageSize,
    margin: Margin,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<u8>, AppError> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let tree_id = Ref::new(2);
    let mut page_ids = Vec::new();

    for (i, spec) in pages.iter().enumerate() {
        // every pdf object needs its own number
        let n = i as i32;
        let page_id = Ref::new(3 + 3 * n);
        let content_id = Ref::new(4 + 3 * n);
        let image_id = Ref::new(5 + 3 * n);
        page_ids.push(page_id);

        let page = render_page(spec, size, margin)?;
        add_page(&mut pdf, tree_id, [page_id, content_id, image_id], size, &page);

        progress(i + 1, pages.len());
    }

    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id).kids(page_ids).count(pages.len() as i32);
    Ok(pdf.finish())
}

// saves to a temp file first and then renames it, so if something fails
// halfway we dont leave a broken pdf behind
pub fn write_atomic(out: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let temp = out.with_extension("pdf.tmp");

    if let Err(e) = fs::write(&temp, bytes) {
        let _ = fs::remove_file(&temp);
        return Err(AppError::write_failed(e));
    }
    if let Err(e) = fs::rename(&temp, out) {
        let _ = fs::remove_file(&temp);
        return Err(AppError::write_failed(e));
    }
    Ok(())
}
