use clap::Parser;
use std::path::PathBuf;

pub mod errors;
pub mod listen;
pub mod scan;
pub mod send;

mod crypto;
mod teleport;
mod utils;

pub const PROTOCOL: u64 = 0x54524f50454c4554;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct SendOpt {
    /// List of filepaths to files that will be teleported
    #[arg(short, long, num_args = ..)]
    input: Vec<PathBuf>,

    /// Destination teleporter host
    #[arg(short, long, default_value = "localhost")]
    dest: String,

    /// Destination teleporter port
    #[arg(short, long, default_value = "9001")]
    port: u16,

    /// Overwrite remote file
    #[arg(short, long)]
    overwrite: bool,

    /// Recurse into directories on send
    #[arg(short, long)]
    recursive: bool,

    /// Encrypt the file transfer using ECDH key-exchange and random keys
    #[arg(short, long)]
    encrypt: bool,

    /// Disable delta transfer (overwrite will transfer entire file)
    #[arg(short, long)]
    no_delta: bool,

    /// Keep path info (recreate directory path on remote server)
    #[arg(short, long)]
    keep_path: bool,

    /// Backup the destination file to a ".bak" extension if it exists and is being overwritten (consecutive runs will replace the *.bak file)
    #[arg(short, long)]
    backup: bool,

    /// If the destination file exists, append a ".1" (or next available number) to the filename instead of overwriting
    #[arg(short, long)]
    filename_append: bool,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct ListenOpt {
    /// Allow absolute and relative file paths for transfers (server only) [WARNING: potentially dangerous option, use at your own risk!]
    #[arg(long)]
    allow_dangerous_filepath: bool,

    /// Require encryption for incoming connections to the server
    #[arg(short, long)]
    must_encrypt: bool,

    /// Port to listen on
    #[arg(short, long, default_value = "9001")]
    port: u16,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct ScanOpt {
    /// Port to scan for
    #[arg(short, long, default_value = "9001")]
    port: u16,
}
