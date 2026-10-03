use crate::error::AppError;
use crate::imaging;
use crate::pdf::{self, Margin, PageSize, PageSpec};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::async_runtime::spawn_blocking;
use tauri::ipc::Channel;

#[derive(Serialize)]
pub struct PhotoInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub thumb: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub pages: Vec<PageSpec>,
    pub page_size: PageSize,
    pub margin: Margin,
    pub out_path: String,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

#[derive(Serialize)]
pub struct ExportSummary {
    pub pages: usize,
    pub bytes: u64,
}

// photo work is slow so run it on a background thread
async fn run_in_background<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    match spawn_blocking(work).await {
        Ok(result) => result,
        Err(e) => Err(AppError::new("internal", None, e)),
    }
}

pub fn import_photo_sync(path: &Path) -> Result<PhotoInfo, AppError> {
    let (thumb_bytes, width, height) = imaging::thumbnail(path)?;
    let thumb = format!("data:image/jpeg;base64,{}", STANDARD.encode(thumb_bytes));

    Ok(PhotoInfo {
        name: imaging::file_name(path),
        width,
        height,
        thumb,
    })
}

#[tauri::command]
pub async fn import_photo(path: String) -> Result<PhotoInfo, AppError> {
    run_in_background(move || import_photo_sync(Path::new(&path))).await
}

#[tauri::command]
pub async fn export_pdf(
    req: ExportRequest,
    on_progress: Channel<Progress>,
) -> Result<ExportSummary, AppError> {
    run_in_background(move || {
        if req.pages.is_empty() {
            return Err(AppError::new("no_pages", None, "nothing to export"));
        }

        let total = req.pages.len();
        let bytes = pdf::build(&req.pages, req.page_size, req.margin, |done, total| {
            let _ = on_progress.send(Progress { done, total });
        })?;
        pdf::write_atomic(Path::new(&req.out_path), &bytes)?;

        Ok(ExportSummary {
            pages: total,
            bytes: bytes.len() as u64,
        })
    })
    .await
}
