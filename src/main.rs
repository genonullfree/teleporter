use blake3::Hash;
use semver::Version;
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
use structopt::StructOpt;

mod client;
mod crypto;
mod server;
mod teleport;
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

    /// Encrypt the file transfer
    #[structopt(short, long)]
    encrypt: bool,

    /// Disable delta transfer (overwrite always overwrites completely)
    #[structopt(short, long)]
    no_delta: bool,

    /// Keep path info (recreate directory path on remote server)
    #[structopt(short, long)]
    keep_path: bool,
}

const PROTOCOL: u64 = 0x54524f50454c4554;
const VERSION: &str = env!("CARGO_PKG_VERSION");

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
