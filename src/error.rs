use std::{io, str::Utf8Error};

#[derive(Debug)]
pub struct QvdError {
    kind: QvdErrorKind,
    message: String,
}

#[derive(Debug)]
pub enum QvdErrorKind {
    ReadFile,
    Utf8Error
}

impl From<io::Error> for QvdError {
    fn from(value: io::Error) -> Self {
        QvdError { kind: QvdErrorKind::ReadFile, message: value.to_string() }
    }
}

impl From<Utf8Error> for QvdError {
    fn from(value: Utf8Error) -> Self {
        QvdError { kind: QvdErrorKind::Utf8Error, message: value.to_string() }
    }
}
