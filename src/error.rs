use std::io;

#[derive(Debug)]
pub struct QvdError {
    kind: QvdErrorKind,
    message: String,
}

#[derive(Debug)]
pub enum QvdErrorKind {
    ReadFile
}

impl From<io::Error> for QvdError {
    fn from(value: io::Error) -> Self {
        QvdError { kind: QvdErrorKind::ReadFile, message: value.to_string() }
    }
}
