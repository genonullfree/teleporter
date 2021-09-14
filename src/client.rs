use crate::utils::print_updates;
use crate::*;

/// Client function sends filename and file data for each filepath
pub fn run(opt: Opt) -> Result<()> {
    // For each filepath in the input vector...
    for (num, item) in opt.input.iter().enumerate() {
        let filepath = item.to_str().unwrap();
        let filename = item.file_name().unwrap();

        // Validate file
        let file = File::open(&filepath).expect("Failed to open file");
        let meta = file.metadata().expect("Failed to read metadata");
        let header = TeleportInit {
            filenum: (num + 1) as u64,
            totalfiles: opt.input.len() as u64,
            filesize: meta.len(),
            filename: filename.to_str().unwrap().to_string(),
            chmod: meta.permissions().mode(),
            overwrite: opt.overwrite,
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
        stream
            .write(&serial.as_bytes())
            .expect("Failed to write to stream");

        let recv = match recv_ack(&stream) {
            Some(t) => t,
            None => {
                println!("Receive TeleportResponse timed out");
                return Ok(());
            }
        };

        if match recv.ack {
            TeleportStatus::NoOverwrite
            | TeleportStatus::NoPermission
            | TeleportStatus::NoSpace => true,
            _ => false,
        } {
            println!("Error: received {:?}", recv.ack);
            return Ok(());
        }

        // Send file data
        let _ = send(stream, file, header);

        println!(" done!");
    }
    Ok(())
}

fn recv_ack(mut stream: &TcpStream) -> Option<TeleportResponse> {
    let mut buf: [u8; 4096] = [0; 4096];

    // Receive ACK that the server is ready for data
    let len = stream
        .read(&mut buf)
        .expect("Failed to receive TeleportResponse");
    let fix = &buf[..len];
    let resp: TeleportResponse =
        serde_json::from_str(str::from_utf8(&fix).unwrap()).expect("Cannot parse TeleportResponse");

    Some(resp)
}

/// Send function receives the ACK for data and sends the file data
fn send(mut stream: TcpStream, mut file: File, header: TeleportInit) -> Result<()> {
    let mut buf: [u8; 4096] = [0; 4096];

    // Send file data
    let mut sent = 0;
    loop {
        // Read a chunk of the file
        let len = file.read(&mut buf).expect("Failed to read file");

        // If a length of 0 was read, we're done sending
        if len == 0 {
            break;
        }

        let data = &buf[..len];

        // Send that data chunk
        stream.write_all(&data).expect("Failed to send data");
        stream.flush().expect("Failed to flush");

        sent += len;
        print_updates(sent as f64, &header);
    }

    Ok(())
}
