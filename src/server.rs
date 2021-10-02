use crate::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Server function sets up a listening socket for any incoming connnections
pub fn run(opt: Opt) -> Result<(), Error> {
    // Bind to all interfaces on specified Port
    let listener = match TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, opt.port))) {
        Ok(l) => l,
        Err(s) => {
            println!("Error binding to port: {:?}", &opt.port);
            return Err(s);
        }
    };

    println!(
        "Teleport Server {} listening for connections on 0.0.0.0:{}",
        VERSION, &opt.port
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

fn send_ack(ack: TeleportInitAck, mut stream: &TcpStream) -> Result<(), Error> {
    // Encode and send response
    let serial_resp = ack.serialize();
    match stream.write(&serial_resp) {
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
fn recv(mut stream: TcpStream, recv_list: Arc<Mutex<Vec<String>>>) -> Result<(), Error> {
    let ip = stream.peer_addr().unwrap();
    let mut file: File;

    // Receive header first
    let mut name_buf: [u8; 4096] = [0; 4096];
    let len = stream.read(&mut name_buf)?;
    let fix = &name_buf[..len];
    let mut header = TeleportInit::new();
    match header.deserialize(fix.to_vec()) {
        Ok(_) => true,
        Err(_) => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Data received did not match expected",
            ))
        }
    };

    if header.protocol != *PROTOCOL || header.version != *VERSION {
        println!(
            "Error: Version mismatch from: {:?}! Us: {}:{} Client: {}:{}",
            ip, PROTOCOL, VERSION, header.protocol, header.version
        );
        let resp = TeleportInitAck::new(TeleportInitStatus::WrongVersion);
        return send_ack(resp, &stream);
    }

    // Test if overwrite is false and file exists
    if !header.overwrite && Path::new(&header.filename).exists() {
        println!(" => Refusing to overwrite file: {}", &header.filename);
        let resp = TeleportInitAck::new(TeleportInitStatus::NoOverwrite);
        return send_ack(resp, &stream);
    }

    // Create recursive dirs
    match fs::create_dir_all(Path::new(&header.filename).parent().unwrap()) {
        Ok(_) => true,
        Err(_) => {
            println!("Error: unable to create directories: {}", &header.filename);
            let resp = TeleportInitAck::new(TeleportInitStatus::NoPermission);
            return send_ack(resp, &stream);
        }
    };

    // Open file for writing
    file = match File::create(&header.filename) {
        Ok(f) => f,
        Err(_) => {
            println!("Error: unable to create file: {}", &header.filename);
            let resp = TeleportInitAck::new(TeleportInitStatus::NoPermission);
            return send_ack(resp, &stream);
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
        Err(_) => {
            println!("Could not set file permissions");
            let resp = TeleportInitAck::new(TeleportInitStatus::NoPermission);
            return send_ack(resp, &stream);
        }
    };

    // Send ready for data ACK
    let resp = TeleportInitAck::new(TeleportInitStatus::Proceed);
    match send_ack(resp, &stream) {
        Ok(_) => true,
        Err(s) => return Err(s),
    };

    let mut recv_data = recv_list.lock().unwrap();
    recv_data.push(header.filename.clone());
    print_list(&recv_data);
    drop(recv_data);

    // Receive file data
    let mut peek: [u8; 4] = [0; 4];
    let mut received: u64 = 0;
    let mut data = Vec::<u8>::new();
    data.resize(5120, 0);
    loop {
        // Read from network connection
        stream.peek(&mut peek)?;
        let mut peek_buf: &[u8] = &peek;
        let chunk_len = peek_buf.read_u32::<LittleEndian>().unwrap() as usize + 4 + 8 + 1;
        let data_slice = &mut data[..chunk_len];
        let len = match stream.read_exact(data_slice) {
            Ok(_) => chunk_len,
            Err(_) => 0,
        };

        if len == 0 {
            if received == header.filesize {
                println!(" => Received file: {} from: {:?}", &header.filename, ip);
            } else {
                println!(" => Error receiving: {}", &header.filename);
            }
            break;
        }

        let mut chunk = TeleportData::new();
        chunk.deserialize(data_slice)?;

        // Write received data to file
        let wrote = match file.write(&chunk.data) {
            Ok(w) => w,
            Err(s) => {
                return Err(s);
            }
        };

        if chunk.length as usize != wrote {
            println!("{:?}", chunk);
            println!(
                "Error writing to file: {} (read: {}, wrote: {}). Out of space?",
                &header.filename, chunk.length, wrote
            );
            break;
        }

        received += chunk.length as u64;

        if received > header.filesize {
            println!(
                "Error: Received {} greater than filesize!",
                received - header.filesize
            );
            break;
        }
    }

    let mut recv_data = recv_list.lock().unwrap();
    recv_data.retain(|x| x != &header.filename);
    print_list(&recv_data);
    drop(recv_data);

    Ok(())
}
