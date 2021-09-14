use crate::utils::print_updates;
use crate::*;
use std::fs;
use std::path::Path;

/// Server function sets up a listening socket for any incoming connnections
pub fn run(opt: Opt) -> Result<()> {
    // Bind to all interfaces on specified Port
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, opt.port)))
        .expect(&format!("Error binding to port: {:?}", opt.port));

    // Listen for incoming connections
    for stream in listener.incoming() {
        // Receive connections in recv function
        thread::spawn(move || {
            recv(stream.unwrap()).unwrap();
        });
    }

    Ok(())
}

fn send_ack(ack: TeleportResponse, mut stream: &TcpStream) -> Result<()> {
    // Encode and send response
    let serial_resp = serde_json::to_string(&ack).unwrap();
    stream
        .write(&serial_resp.as_bytes())
        .expect("Failed to write to stream");

    Ok(())
}

/// Recv receives filenames and file data for a file
fn recv(mut stream: TcpStream) -> Result<()> {
    let ip = stream.peer_addr().unwrap();
    let mut file: File;

    // Receive header first
    let mut name_buf: [u8; 4096] = [0; 4096];
    let len = stream.read(&mut name_buf)?;
    let fix = &name_buf[..len];
    let header: TeleportInit =
        serde_json::from_str(str::from_utf8(&fix).unwrap()).expect("Cannot understand filename");
    println!(
        "Receiving file {}/{}: {:?} (from {})",
        header.filenum, header.totalfiles, header.filename, ip
    );

    // Test if overwrite is false and file exists
    if !header.overwrite && Path::new(&header.filename).exists() {
        println!(" => Refusing to overwrite file: {}", &header.filename);
        let resp = TeleportResponse {
            ack: TeleportStatus::NoOverwrite,
        };
        send_ack(resp, &stream).expect("Failed to send ack");
        return Ok(());
    }

    // Open file for writing
    file = File::create(&header.filename).expect("Could not open file");
    let meta = file.metadata().expect("Could not read file metadata");
    let mut perms = meta.permissions();
    perms.set_mode(header.chmod);
    fs::set_permissions(&header.filename, perms).expect("Could not set file permissions");

    // Send ready for data ACK
    let resp = TeleportResponse {
        ack: TeleportStatus::Proceed,
    };
    send_ack(resp, &stream).expect("Failed to send ack");

    // Receive file data
    let mut buf: [u8; 4096] = [0; 4096];
    let mut received: u64 = 0;
    loop {
        // Read from network connection
        let len = stream.read(&mut buf).expect("Failed to read stream");
        if len == 0 {
            println!(" done!");
            break;
        }
        let data = &buf[..len];

        // Write received data to file
        let wrote = file.write(&data).expect("Failed to write to file");
        if len != wrote {
            println!(
                "Error writing to file: {} (read: {}, wrote: {}",
                &header.filename, len, wrote
            );
            break;
        }

        received += len as u64;
        print_updates(received as f64, &header);
    }

    Ok(())
}
