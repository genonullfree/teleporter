/*
use std::fs::File;
use std::io::{self, Read, Write};
use std::io::{Error, ErrorKind};
use std::io::{Seek, SeekFrom};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::result::Result;
use std::str;
use std::thread;
use std::time::Instant;
*/
use clap::Parser;

use teleporter::listen;
use teleporter::send;
use teleporter::{ListenOpt, SendOpt};

/// Teleporter is a simple application for sending files from Point A to Point B
#[derive(Clone, Debug, Parser, PartialEq, Eq)]
#[command(version)]
pub struct Opt {
    /// Command
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Debug, Parser, PartialEq, Eq)]
pub enum Cmd {
    /// Start a teleporter in server (receiving) mode
    Listen(ListenOpt),
    /// Start a teleporter in client (sending) mode
    Send(SendOpt),
}

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
        Err(s) => println!("Error: {s}"),
    };
}
