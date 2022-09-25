use std::fs::File;
use std::io::{self, Read, Write};
use std::io::{Error, ErrorKind};
use std::io::{Seek, SeekFrom};
use std::net::Ipv4Addr;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::result::Result;
use std::str;
use std::thread;
use std::time::Instant;

use clap::Parser;
use semver::Version;

mod listen;
mod send;
mod crypto;
mod errors;
mod teleport;
mod utils;
use errors::TeleportError;

/// Teleporter is a simple application for sending files from Point A to Point B

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub enum Cmd {
    Listen(ListenOpt),
    Send(SendOpt),
    //Command,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct Opt {
    /// Command
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct SendOpt {
    /// List of filepaths to files that will be teleported
    #[clap(short, long, multiple_values = true, default_value = "")]
    input: Vec<PathBuf>,

    /// Destination teleporter IP address
    #[clap(short, long, default_value = "127.0.0.1")]
    dest: String,

    /// Destination teleporter Port, or Port to listen on
    #[clap(short, long, default_value = "9001")]
    port: u16,

    /// Overwrite remote file
    #[clap(short, long)]
    overwrite: bool,

    /// Recurse into directories on send
    #[clap(short, long)]
    recursive: bool,

    /// Encrypt the file transfer using ECDH key-exchange and random keys
    #[clap(short, long)]
    encrypt: bool,

    /// Disable delta transfer (overwrite will transfer entire file)
    #[clap(short, long)]
    no_delta: bool,

    /// Keep path info (recreate directory path on remote server)
    #[clap(short, long)]
    keep_path: bool,

    /// Backup the destination file to a ".bak" extension if it exists and is being overwritten (consecutive runs will replace the *.bak file)
    #[clap(short, long)]
    backup: bool,

    /// If the destination file exists, append a ".1" (or next available number) to the filename instead of overwriting
    #[clap(short, long)]
    filename_append: bool,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct ListenOpt {
    /// Allow absolute and relative file paths for transfers (server only) [WARNING: potentially dangerous option, use at your own risk!]
    #[clap(long)]
    allow_dangerous_filepath: bool,

    /// Require encryption for incoming connections to the server
    #[clap(short, long)]
    must_encrypt: bool,

    /// Destination teleporter Port, or Port to listen on
    #[clap(short, long, default_value = "9001")]
    port: u16,
}

const PROTOCOL: u64 = 0x54524f50454c4554;
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Process arguments
    let opt = Opt::parse();

    // If the input filepath list is empty, assume we're in server mode
    let out = match opt.cmd {
        Cmd::Listen(l) => listen::run(l),
        // Else, we have files to send so we're in client mode
        Cmd::Send(s) => send::run(s),
    };

    match out {
        Ok(()) => {}
        Err(s) => println!("Error: {}", s),
    };
}
