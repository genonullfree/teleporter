use crate::utils::print_updates;
use crate::*;

/// Client function sends filename and file data for each filepath
pub fn run(opt: Opt) -> Result<(), Error> {
    println!("Teleport Client {}", VERSION);

    // For each filepath in the input vector...
    for (num, item) in opt.input.iter().enumerate() {
        let filepath = item.to_str().unwrap();
        let filename = item.file_name().unwrap();

        // Validate file
        let file = match File::open(&filepath) {
            Ok(f) => f,
            Err(s) => {
                println!("Error opening file: {}", filepath);
                return Err(s);
            }
        };
        let meta = match file.metadata() {
            Ok(m) => m,
            Err(s) => return Err(s),
        };
        let header = TeleportInit {
            protocol: PROTOCOL.to_string(),
            version: VERSION.to_string(),
            filenum: (num + 1) as u64,
            totalfiles: opt.input.len() as u64,
            filesize: meta.len(),
            filename: filename.to_str().unwrap().to_string(),
            chmod: meta.permissions().mode(),
            overwrite: opt.overwrite,
        };

        // Connect to server
        let addr = format!("{}:{}", opt.dest, opt.port);
        let mut stream = match TcpStream::connect(match addr.parse::<SocketAddr>() {
            Ok(a) => a,
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Error with destination address",
                ))
            }
        }) {
            Ok(s) => s,
            Err(s) => {
                println!("Error connecting to: {}:{}", opt.dest, opt.port);
                return Err(s);
            }
        };

        println!(
            "Sending file {}/{}: {:?}",
            header.filenum, header.totalfiles, header.filename
        );

        // Send header first
        match stream.write(&header.serialize()) {
            Ok(_) => true,
            Err(s) => return Err(s),
        };

        // Receive response from server
        let recv = match recv_ack(&stream) {
            Some(t) => t,
            None => {
                println!("Receive TeleportResponse timed out");
                return Ok(());
            }
        };

        // Validate response
        match recv.ack {
            TeleportStatus::Overwrite => {
                println!("The server is overwriting the file: {}", &header.filename)
            }
            TeleportStatus::NoOverwrite => {
                println!(
                    "The server refused to overwrite the file: {}",
                    &header.filename
                );
                continue;
            }
            TeleportStatus::NoPermission => {
                println!(
                    "The server does not have permission to write to this file: {}",
                    &header.filename
                );
                continue;
            }
            TeleportStatus::NoSpace => {
                println!(
                    "The server has no space available to write the file: {}",
                    &header.filename
                );
                continue;
            }
            TeleportStatus::WrongVersion => {
                println!("Error: The server does not speak {} {}", PROTOCOL, VERSION);
                break;
            }
            _ => (),
        };

        // Send file data
        match send(stream, file, header) {
            Ok(_) => true,
            Err(s) => return Err(s),
        };

        println!(" done!");
    }
    Ok(())
}

fn recv_ack(mut stream: &TcpStream) -> Option<TeleportResponse> {
    let mut buf: [u8; 4096] = [0; 4096];

    // Receive ACK that the server is ready for data
    let len = match stream.read(&mut buf) {
        Ok(l) => l,
        Err(_) => return None,
    };

    let fix = &buf[..len];
    let mut resp = TeleportResponse::new(TeleportStatus::WrongVersion);
    match resp.deserialize(fix.to_vec()) {
        Ok(_) => true,
        Err(_) => return None,
    };

    Some(resp)
}

/// Send function receives the ACK for data and sends the file data
fn send(mut stream: TcpStream, mut file: File, header: TeleportInit) -> Result<(), Error> {
    let mut buf: [u8; 4096] = [0; 4096];

    // Send file data
    let mut sent = 0;
    loop {
        // Read a chunk of the file
        let len = match file.read(&mut buf) {
            Ok(l) => l,
            Err(s) => return Err(s),
        };

        // If a length of 0 was read, we're done sending
        if len == 0 {
            break;
        }

        let data = &buf[..len];

        // Send the data chunk
        match stream.write_all(&data) {
            Ok(_) => true,
            Err(s) => return Err(s),
        };
        match stream.flush() {
            Ok(_) => true,
            Err(s) => return Err(s),
        };

        sent += len;
        print_updates(sent as f64, &header);
    }

    Ok(())
}
