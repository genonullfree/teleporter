use crate::teleport::*;
use crate::teleport::{TeleportFeatures, TeleportStatus};
use crate::teleport::{TeleportInit, TeleportInitAck};
use crate::*;
use std::fs;
use std::fs::OpenOptions;
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
        "Teleporter Server {} listening for connections on 0.0.0.0:{}",
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

fn send_ack(
    ack: TeleportInitAck,
    mut stream: &mut TcpStream,
    enc: &Option<TeleportEnc>,
) -> Result<(), Error> {
    // Encode and send response
    utils::send_packet(&mut stream, TeleportAction::InitAck, enc, ack.serialize())
}

fn print_list(list: &MutexGuard<Vec<String>>) {
    if list.len() == 0 {
        print!("\rListening...");
    } else {
        print!("\rReceiving: {:?}", list);
    }
    io::stdout().flush().unwrap();
}

/// Recv receives filenames and file data for a file
fn recv(mut stream: TcpStream, recv_list: Arc<Mutex<Vec<String>>>) -> Result<(), Error> {
    let start_time = Instant::now();
    let ip = stream.peer_addr().unwrap();
    let mut file: File;

    let mut enc: Option<TeleportEnc> = None;

    // Receive header first
    let mut packet = utils::recv_packet(&mut stream, &None)?;
    if packet.action == TeleportAction::Ecdh as u8 {
        let mut ctx = TeleportEnc::new();
        ctx.deserialize(&packet.data)?;
        utils::send_packet(&mut stream, TeleportAction::EcdhAck, &None, ctx.serialize())?;
        enc = Some(ctx);
        packet = utils::recv_packet(&mut stream, &enc)?;
    }

    let mut header = TeleportInit::new(TeleportFeatures::NewFile);
    if packet.action == TeleportAction::Init as u8 {
        header.deserialize(&packet.data)?;
    } else {
        let resp = TeleportInitAck::new(TeleportStatus::EncryptionError);
        return send_ack(resp, &mut stream, &enc);
    }

    let mut filename: String = header.filename.iter().cloned().collect::<String>();
    let features: u32 = header.features;

    let version = Version::parse(VERSION).unwrap();
    let compatible =
        { version.major as u16 == header.version[0] && version.minor as u16 == header.version[1] };

    if !compatible {
        println!(
            "Error: Version mismatch from: {:?}! Us:{} Client:{:?}",
            ip, VERSION, header.version
        );
        let resp = TeleportInitAck::new(TeleportStatus::WrongVersion);
        return send_ack(resp, &mut stream, &enc);
    }

    // Remove any preceeding '/'
    if filename.starts_with('/') {
        filename.remove(0);
    }

    // Prohibit directory traversal
    filename = filename.replace("../", "");

    // Test if overwrite is false and file exists
    if features & TeleportFeatures::Overwrite as u32 != TeleportFeatures::Overwrite as u32
        && Path::new(&filename).exists()
    {
        println!(" => Refusing to overwrite file: {:?}", &filename);
        let resp = TeleportInitAck::new(TeleportStatus::NoOverwrite);
        return send_ack(resp, &mut stream, &enc);
    }

    // Create recursive dirs
    if fs::create_dir_all(Path::new(&filename).parent().unwrap()).is_err() {
        println!("Error: unable to create directories: {:?}", &filename);
        let resp = TeleportInitAck::new(TeleportStatus::NoPermission);
        return send_ack(resp, &mut stream, &enc);
    };

    // Open file for writing
    file = match OpenOptions::new().read(true).write(true).open(&filename) {
        Ok(f) => f,
        Err(_) => match File::create(&filename) {
            Ok(f) => f,
            Err(_) => {
                println!("Error: unable to create file: {:?}", &filename);
                let resp = TeleportInitAck::new(TeleportStatus::NoPermission);
                return send_ack(resp, &mut stream, &enc);
            }
        },
    };
    let meta = file.metadata()?;
    let mut perms = meta.permissions();
    perms.set_mode(header.chmod);
    if fs::set_permissions(&filename, perms).is_err() {
        println!("Could not set file permissions");
        let resp = TeleportInitAck::new(TeleportStatus::NoPermission);
        return send_ack(resp, &mut stream, &enc);
    };

    // Send ready for data ACK
    let mut resp = TeleportInitAck::new(TeleportStatus::Proceed);
    utils::add_feature(&mut resp.features, TeleportFeatures::NewFile)?;

    // Add file to list
    let mut recv_data = recv_list.lock().unwrap();
    recv_data.push(filename.clone());
    print_list(&recv_data);
    drop(recv_data);

    // If overwrite and file exists, build TeleportDelta
    file.set_len(header.filesize)?;
    if meta.len() > 0 {
        utils::add_feature(&mut resp.features, TeleportFeatures::Overwrite)?;
        if features & TeleportFeatures::Delta as u32 == TeleportFeatures::Delta as u32 {
            utils::add_feature(&mut resp.features, TeleportFeatures::Delta)?;
            resp.delta = match utils::calc_delta_hash(&file) {
                Ok(d) => Some(d),
                _ => None,
            };
        }
    }

    send_ack(resp, &mut stream, &enc)?;

    // Receive file data
    let mut received: u64 = 0;
    loop {
        // Read from network connection
        let packet = utils::recv_packet(&mut stream, &enc)?;
        let mut chunk = TeleportData::new();
        chunk.deserialize(&packet.data)?;

        if chunk.data_len == 0 {
            if received == header.filesize
                || (header.filesize == chunk.offset && chunk.data_len == 0)
            {
                let duration = start_time.elapsed();
                let speed =
                    (header.filesize as f64 * 8.0) / duration.as_secs() as f64 / 1024.0 / 1024.0;
                println!(
                    " => Received file: {:?} (from: {:?} v{:?}) ({:.2?} @ {:.3} Mbps)",
                    &filename, ip, &header.version, duration, speed
                );
            } else {
                println!(" => Error receiving: {:?}", &filename);
            }
            break;
        }

        // Seek to offset
        file.seek(SeekFrom::Start(chunk.offset))?;

        // Write received data to file
        let wrote = file.write(&chunk.data)?;

        if chunk.data_len as usize != wrote {
            println!(
                "Error writing to file: {:?} (read: {}, wrote: {}). Out of space?",
                &filename, chunk.data_len, wrote
            );
            break;
        }

        received = chunk.offset;
        received += chunk.data_len as u64;

        if received > header.filesize {
            println!(
                "Error: Received {} greater than filesize!",
                received - header.filesize
            );
            break;
        }
    }

    let mut recv_data = recv_list.lock().unwrap();
    recv_data.retain(|x| x != &filename);
    print_list(&recv_data);
    drop(recv_data);

    Ok(())
}
