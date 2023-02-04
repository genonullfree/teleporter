use std::io;
use std::num::ParseIntError;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TeleportError {
    #[error("IoError: {0}")]
    Io(#[from] io::Error),

    #[error("Error in conversion of Utf8")]
    Utf8Error(#[from] Utf8Error),

    #[error("Error in conversion of Utf8")]
    FromUtf8Error(#[from] FromUtf8Error),

    #[error("Error in conversion of Int")]
    ParseIntError(#[from] ParseIntError),

    #[error("Error with destination address")]
    InvalidDest,

    #[error("Invalid Protocol header received")]
    InvalidProtocol,

    #[error("Invalid file name")]
    InvalidFileName,
}
