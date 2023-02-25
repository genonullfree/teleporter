use clap::Parser;

use teleporter::{listen, send, scan};
use teleporter::{ListenOpt, SendOpt, ScanOpt};

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
    /// Scan all network devices for any reachable Teleport listeners
    Scan(ScanOpt),
}

fn main() {
    // Process arguments
    let opt = Opt::parse();

    // Execute command
    let out = match opt.cmd {
        Cmd::Listen(l) => listen::run(l),
        Cmd::Send(s) => send::run(s),
        Cmd::Scan(s) => scan::run(s),
    };

    // Display any errors
    match out {
        Ok(()) => {}
        Err(s) => println!("Error: {s}"),
    };
}
