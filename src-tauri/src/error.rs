use serde::Serialize;
use std::io;

#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub file: Option<String>,
    pub detail: String,
}

impl AppError {
    pub fn new(code: &'static str, file: Option<&str>, detail: impl ToString) -> AppError {
        AppError {
            code,
            file: file.map(|f| f.to_string()),
            detail: detail.to_string(),
        }
    }

    pub fn read_failed(e: io::Error, file: &str) -> AppError {
        AppError::new("unreadable", Some(file), e)
    }

    pub fn write_failed(e: io::Error) -> AppError {
        if e.kind() == io::ErrorKind::StorageFull {
            AppError::new("disk_full", None, e)
        } else {
            AppError::new("write_failed", None, e)
        }
    }

    pub fn from_image(e: image::ImageError, file: &str) -> AppError {
        match e {
            image::ImageError::Limits(_) => AppError::new("too_large", Some(file), e),
            image::ImageError::Unsupported(_) => AppError::new("unsupported", Some(file), e),
            image::ImageError::IoError(io_error) => AppError::read_failed(io_error, file),
            _ => AppError::new("corrupt", Some(file), e),
        }
    }
}
