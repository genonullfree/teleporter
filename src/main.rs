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

mod crypto;
mod errors;
mod listen;
mod send;
mod teleport;
mod utils;
use errors::TeleportError;

/// Teleporter is a simple application for sending files from Point A to Point B

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub enum Cmd {
    /// Start a teleporter in server (receiving) mode
    Listen(ListenOpt),
    /// Start a teleporter in client (sending) mode
    Send(SendOpt),
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct Opt {
    /// Command
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub struct SendOpt {
    /// List of filepaths to files that will be teleported
    #[arg(short, long, num_args = ..)]
    input: Vec<PathBuf>,

    /// Destination teleporter IP address
    #[arg(short, long, default_value_t = Ipv4Addr::LOCALHOST)]
    dest: Ipv4Addr,

    /// Destination teleporter Port
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

const PROTOCOL: u64 = 0x54524f50454c4554;
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Process arguments
    let opt = Opt::parse();

    // Execute command
    let out = match opt.cmd {
        Cmd::Listen(l) => listen::run(l),
        Cmd::Send(s) => send::run(s),
    };

    // Display any errors
    match out {
        Ok(()) => {}
        Err(s) => println!("Error: {}", s),
    };
}
