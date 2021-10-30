use blake3::Hash;
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
use structopt::StructOpt;

mod client;
mod server;
mod utils;

/// Teleporter is a simple application for sending files from Point A to Point B

#[derive(Debug, StructOpt)]
pub struct Opt {
    /// List of filepaths to files that will be teleported
    #[structopt(short, long, parse(from_os_str), default_value = "")]
    input: Vec<PathBuf>,

    /// Destination teleporter IP address
    #[structopt(short, long, default_value = "127.0.0.1")]
    dest: String,

    /// Destination teleporter Port, or Port to listen on
    #[structopt(short, long, default_value = "9001")]
    port: u16,

    /// Overwrite remote file
    #[structopt(short, long)]
    overwrite: bool,

    /// Recurse into directories on send
    #[structopt(short, long)]
    recursive: bool,
}

const PROTOCOL: &str = "TELEPORT";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct TeleportInit {
    protocol: String,
    version: String,
    filename: String,
    filenum: u64,
    totalfiles: u64,
    filesize: u64,
    chmod: u32,
    overwrite: bool,
}

#[derive(Debug, PartialEq)]
pub struct TeleportInitAck {
    ack: TeleportInitStatus,
    version: String,
    delta: Option<TeleportDelta>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TeleportDelta {
    size: u64,
    delta_size: u64,
    csum: Hash,
    delta_csum: Vec<Hash>,
}

#[derive(Debug, PartialEq)]
pub struct TeleportData {
    length: u32,
    offset: u64,
    data: Vec<u8>,
}

/// TeleportInitStatus type when header is received and ready to receive file data or not
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TeleportInitStatus {
    Proceed,      // Success
    Overwrite,    // Success, delta overwrite
    NoOverwrite,  // Error
    NoSpace,      // Error
    NoPermission, // Error
    WrongVersion, // Error
}

fn main() {
    // Process arguments
    let opt = Opt::from_args();
    let out;

    // If the input filepath list is empty, assume we're in server mode
    if opt.input.len() == 1 && opt.input[0].to_str().unwrap() == "" {
        out = server::run(opt);
    // Else, we have files to send so we're in client mode
    } else {
        out = client::run(opt);
    }
    match out {
        Ok(()) => {}
        Err(s) => println!("Error: {}", s),
    };
}
