use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Result;
use std::io::{self, Read, Write};
use std::net::Ipv4Addr;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::str;
use structopt::StructOpt;

/// Teleport is a simple application for sending files from Point A to Point B

#[derive(Debug, StructOpt)]
struct Opt {
    /// List of filepaths to files that will be teleported
    #[structopt(short, long, parse(from_os_str), default_value = "")]
    input: Vec<PathBuf>,

    /// Destination teleport IP address
    #[structopt(short, long, default_value = "127.0.0.1")]
    dest: String,

    /// Destination teleport Port, or Port to listen on
    #[structopt(short, long, default_value = "9001")]
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct TeleProto {
    filenum: u64,
    totalfiles: u64,
    filesize: u64,
    filename: String,
}

struct SizeUnit {
    partial: f64,
    partial_unit: char,
    total: f64,
    total_unit: char,
}

/// ACK response type when filename is received and ready to receive file data
const ACK: [u8; 3] = ['A' as u8, 'C' as u8, 'K' as u8];

fn main() {
    // Process arguments
    let opt = Opt::from_args();

    // If the input filepath list is empty, assume we're in server mode
    if opt.input.len() == 1 && opt.input[0].to_str().unwrap() == "" {
        println!("Server mode, listening for connections");
        let _ = server(opt);
    // Else, we have files to send so we're in client mode
    } else {
        println!("Client mode");
        let _ = client(opt);
    }
}

/// Client function sends filename and file data for each filepath
fn client(opt: Opt) -> Result<()> {
    // For each filepath in the input vector...
    for (num, item) in opt.input.iter().enumerate() {
        let filepath = item.to_str().unwrap();
        let filename = item.file_name().unwrap();

        // Validate file
        let file = File::open(&filepath).expect("Failed to open file");
        let meta = file.metadata().expect("Failed to read metadata");
        let header = TeleProto {
            filenum: (num + 1) as u64,
            totalfiles: opt.input.len() as u64,
            filesize: meta.len(),
            filename: filename.to_str().unwrap().to_string(),
        };

        // Connect to server
        let addr = format!("{}:{}", opt.dest, opt.port);
        let mut stream = TcpStream::connect(
            addr.parse::<SocketAddr>()
                .expect(&format!("Error with dest: {}", addr)),
        )
        .expect(&format!("Error connecting to: {:?}", opt.dest));

        println!(
            "Sending file {}/{}: {:?}",
            header.filenum, header.totalfiles, header.filename
        );

        // Send header first
        let serial = serde_json::to_string(&header).unwrap();
        //let serial = &TeleProto::serialize(&header).unwrap();
        stream
            .write(&serial.as_bytes())
            .expect("Failed to write to stream");

        // Send file data
        let _ = send(stream, file, header);

        println!(" done!");
    }
    Ok(())
}

/// Send function receives the ACK for data and sends the file data
fn send(mut stream: TcpStream, mut file: File, header: TeleProto) -> Result<()> {
    let mut buf: [u8; 4096] = [0; 4096];

    // Receive ACK that the server is ready for data
    stream.read(&mut buf).expect("Failed to receive ACK");
    for (i, v) in ACK.iter().enumerate() {
        if v != &buf[i] {
            return Ok(());
        }
    }

    // Send file data
    let mut sent = 0;
    loop {
        // Read a chunk of the file
        let len = file.read(&mut buf).expect("Failed to read file");

        // If a length of 0 was read, we're done sending
        if len == 0 {
            break;
        }

        // Send that data chunk
        let data = &buf[..len];
        let wrote = stream.write(data).expect("Failed to send data");
        if len != wrote {
            println!("Error sending data");
            break;
        }

        sent += len;
        print_updates(sent as f64, &header);
    }

    Ok(())
}

/// Server function sets up a listening socket for any incoming connnections
fn server(opt: Opt) -> Result<()> {
    // Bind to all interfaces on specified Port
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, opt.port)))
        .expect(&format!("Error binding to port: {:?}", opt.port));

    // Listen for incoming connections
    for stream in listener.incoming() {
        // Receive connections in recv function
        recv(stream?)?;
    }

    Ok(())
}

/// Recv receives filenames and file data for a file
fn recv(mut stream: TcpStream) -> Result<()> {
    let ip = stream.peer_addr().unwrap();

    // Receive header first
    let mut name_buf: [u8; 4096] = [0; 4096];
    let len = stream.read(&mut name_buf)?;
    let fix = &name_buf[..len];
    let header: TeleProto =
        serde_json::from_str(str::from_utf8(&fix).unwrap()).expect("Cannot understand filename");
    println!(
        "Receiving file {}/{}: {:?} (from {})",
        header.filenum, header.totalfiles, header.filename, ip
    );

    // Send ready for data ACK
    stream.write(&ACK).expect("Failed to ACK");

    // Receive file data
    let mut file = File::create(&header.filename).expect("Could not open file");
    let mut buf: [u8; 4096] = [0; 4096];
    let mut received: u64 = 0;
    loop {
        // Read from network connection
        let len = stream.read(&mut buf).expect("Failed to read");

        // A receive of length 0 means the transfer is complete
        if len == 0 {
            println!(" done!");
            break;
        }

        // Write received data to file
        let data = &buf[..len];
        let wrote = file.write(data).expect("Failed to write to file");
        if len != wrote {
            println!("Error writing to file: {}", &header.filename);
            break;
        }

        received += len as u64;
        print_updates(received as f64, &header);
    }

    Ok(())
}

fn print_updates(received: f64, header: &TeleProto) {
    let percent: f64 = (received as f64 / header.filesize as f64) * 100f64;
    let units = convert_units(received as f64, header.filesize as f64);
    print!(
        "\r => {:>8.03}{} of {:>8.03}{} ({:02.02}%)",
        units.partial, units.partial_unit, units.total, units.total_unit, percent
    );
    io::stdout().flush().unwrap();
}

fn convert_units(mut partial: f64, mut total: f64) -> SizeUnit {
    let unit = ['B', 'K', 'M', 'G', 'T'];
    let mut out = SizeUnit {
        partial: 0.0,
        partial_unit: 'B',
        total: 0.0,
        total_unit: 'B',
    };

    let mut count = 0;
    loop {
        if (total / 1024.0) > 1.0 {
            count += 1;
            total /= 1024.0;
        } else {
            break;
        }
        if count == unit.len() - 1 {
            break;
        }
    }
    out.total = total;
    out.total_unit = unit[count];

    count = 0;
    loop {
        if (partial / 1024.0) > 1.0 {
            count += 1;
            partial /= 1024.0;
        } else {
            break;
        }
        if count == unit.len() - 1 {
            break;
        }
    }
    out.partial = partial;
    out.partial_unit = unit[count];
    out
}
