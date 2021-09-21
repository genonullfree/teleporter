use std::fs::File;
use std::io::{self, Read, Write};
use std::io::{Error, ErrorKind};
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

/// Teleport is a simple application for sending files from Point A to Point B

#[derive(Debug, StructOpt)]
pub struct Opt {
    /// List of filepaths to files that will be teleported
    #[structopt(short, long, parse(from_os_str), default_value = "")]
    input: Vec<PathBuf>,

    /// Destination teleport IP address
    #[structopt(short, long, default_value = "127.0.0.1")]
    dest: String,

    /// Destination teleport Port, or Port to listen on
    #[structopt(short, long, default_value = "9001")]
    port: u16,

    /// Overwrite remote file
    #[structopt(short, long)]
    overwrite: bool,
}

const PROTOCOL: &str = "TELEPORT";
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct TeleportInit {
    protocol: String,
    version: String,
    filenum: u64,
    totalfiles: u64,
    filesize: u64,
    filename: String,
    chmod: u32,
    overwrite: bool,
}

#[derive(Debug)]
pub struct TeleportResponse {
    ack: TeleportStatus,
}

/// TeleportStatus type when header is received and ready to receive file data or not
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TeleportStatus {
    Proceed,      // Success
    Overwrite,    // Success
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
        Ok(()) => return,
        Err(s) => {
            println!("Error: {}", s);
            return;
        }
    };
}
