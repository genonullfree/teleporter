use crate::*;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Server function sets up a listening socket for any incoming connnections
pub fn run(opt: Opt) -> Result<()> {
    // Bind to all interfaces on specified Port
    let listener = match TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, opt.port))) {
        Ok(l) => l,
        Err(s) => {
            println!("Error binding to port: {:?}", &opt.port);
            return Err(s);
        }
    };

    println!(
        "Teleport Server listening for connections on 0.0.0.0:{}",
        &opt.port
    );

    let recv_list = Arc::new(Mutex::new(Vec::<String>::new()));

    // Listen for incoming connections
    for stream in listener.incoming() {
        let s = match stream {
            Ok(s) => s,
            _ => continue,
        };
        // Receive connections in recv function
        let recv_list_clone = Arc::clone(&recv_list);
        thread::spawn(move || {
            recv(s, recv_list_clone).unwrap();
        });
    }

    Ok(())
}

fn send_ack(ack: TeleportResponse, mut stream: &TcpStream) -> Result<()> {
    // Encode and send response
    let serial_resp = serde_json::to_string(&ack).unwrap();
    match stream.write(&serial_resp.as_bytes()) {
        Ok(_) => true,
        Err(s) => return Err(s),
    };

    Ok(())
}

fn print_list(list: &MutexGuard<Vec<String>>) {
    print!("\rReceiving: {:?}", list);
    io::stdout().flush().unwrap();
}

/// Recv receives filenames and file data for a file
fn recv(mut stream: TcpStream, recv_list: Arc<Mutex<Vec<String>>>) -> Result<()> {
    let ip = stream.peer_addr().unwrap();
    let mut file: File;

    // Receive header first
    let mut name_buf: [u8; 4096] = [0; 4096];
    let len = stream.read(&mut name_buf)?;
    let fix = &name_buf[..len];
    let header: TeleportInit = match serde_json::from_str(str::from_utf8(&fix).unwrap()) {
        Ok(s) => s,
        Err(_) => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Data received did not match expected",
            ))
        }
    };

    let mut recv_data = recv_list.lock().unwrap();
    recv_data.push(header.filename.clone());
    print_list(&recv_data);
    drop(recv_data);

    // Test if overwrite is false and file exists
    if !header.overwrite && Path::new(&header.filename).exists() {
        println!(" => Refusing to overwrite file: {}", &header.filename);
        let resp = TeleportResponse {
            ack: TeleportStatus::NoOverwrite,
        };
        return send_ack(resp, &stream);
    }

    // Open file for writing
    file = match File::create(&header.filename) {
        Ok(f) => f,
        Err(s) => {
            println!("Error: unable to create file: {}", &header.filename);
            return Err(s);
        }
    };
    let meta = match file.metadata() {
        Ok(m) => m,
        Err(s) => return Err(s),
    };
    let mut perms = meta.permissions();
    perms.set_mode(header.chmod);
    match fs::set_permissions(&header.filename, perms) {
        Ok(_) => true,
        Err(s) => {
            println!("Could not set file permissions");
            return Err(s);
        }
    };

    // Send ready for data ACK
    let resp = TeleportResponse {
        ack: TeleportStatus::Proceed,
    };
    match send_ack(resp, &stream) {
        Ok(_) => true,
        Err(s) => return Err(s),
    };

    // Receive file data
    let mut buf: [u8; 4096] = [0; 4096];
    let mut received: u64 = 0;
    loop {
        // Read from network connection
        let len = match stream.read(&mut buf) {
            Ok(l) => l,
            Err(s) => return Err(s),
        };

        if len == 0 {
            if received != header.filesize {
                println!(" => Error receiving: {}", &header.filename);
            } else {
                println!(" => Received file: {} from: {:?}", &header.filename, ip);
                let mut recv_data = recv_list.lock().unwrap();
                recv_data.retain(|x| x != &header.filename);
                print_list(&recv_data);
                drop(recv_data);
            }
            break;
        }
        let data = &buf[..len];

        // Write received data to file
        let wrote = match file.write(&data) {
            Ok(w) => w,
            Err(s) => return Err(s),
        };

        if len != wrote {
            println!(
                "Error writing to file: {} (read: {}, wrote: {}",
                &header.filename, len, wrote
            );
            break;
        }

        received += len as u64;
    }

    Ok(())
}
