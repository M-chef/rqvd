use std::{fmt::Display, io, num::ParseIntError, str::Utf8Error, string::FromUtf8Error};

#[derive(Debug)]
pub enum QvdErrorKind {
    ReadFile,
    Utf8Error,
    ParseError,
}

#[derive(Debug)]
pub struct QvdError {
    pub(crate) kind: QvdErrorKind,
    pub(crate) message: String,
}

impl Display for QvdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}, {}", self.kind, self.message)
    }
}

impl std::error::Error for QvdError {}

impl From<io::Error> for QvdError {
    fn from(value: io::Error) -> Self {
        QvdError {
            kind: QvdErrorKind::ReadFile,
            message: value.to_string(),
        }
    }
}

impl From<Utf8Error> for QvdError {
    fn from(value: Utf8Error) -> Self {
        QvdError {
            kind: QvdErrorKind::Utf8Error,
            message: value.to_string(),
        }
    }
}

impl From<FromUtf8Error> for QvdError {
    fn from(value: FromUtf8Error) -> Self {
        QvdError {
            kind: QvdErrorKind::Utf8Error,
            message: value.to_string(),
        }
    }
}

impl From<ParseIntError> for QvdError {
    fn from(value: ParseIntError) -> Self {
        QvdError {
            kind: QvdErrorKind::ParseError,
            message: value.to_string(),
        }
    }
}

impl From<quick_xml::DeError> for QvdError {
    fn from(value: quick_xml::DeError) -> Self {
        QvdError {
            kind: QvdErrorKind::ReadFile,
            message: value.to_string(),
        }
    }
}
