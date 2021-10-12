use crate::utils::print_updates;
use crate::*;
use std::path::Path;

fn get_file_list(opt: &Opt) -> Vec<String> {
    let mut files = Vec::<String>::new();

    for item in opt.input.iter() {
        if opt.recursive && item.is_dir() {
            let mut tmp = match scope_dir(&item.to_path_buf()) {
                Ok(t) => t,
                Err(_) => {
                    println!("Error: Cannot read item: {:?}", item);
                    continue;
                }
            };
            files.append(&mut tmp);
        } else if item.is_file() {
            files.push(item.to_str().unwrap().to_string());
        }
    }

    files
}

fn scope_dir(dir: &Path) -> Result<Vec<String>, Error> {
    let path = Path::new(&dir);
    let mut files = Vec::<String>::new();

    for entry in path.read_dir().unwrap() {
        if entry.as_ref().unwrap().file_type().unwrap().is_dir() {
            if entry.as_ref().unwrap().path() == *dir {
                continue;
            }

            let mut tmp = match scope_dir(&entry.as_ref().unwrap().path()) {
                Ok(t) => t,
                Err(_) => {
                    println!("Error: Cannot read dir: {:?}", entry);
                    continue;
                }
            };
            files.append(&mut tmp);
        } else if entry.as_ref().unwrap().file_type().unwrap().is_file() {
            files.push(entry.unwrap().path().to_str().unwrap().to_string());
        }
    }

    Ok(files)
}

/// Client function sends filename and file data for each filepath
pub fn run(opt: Opt) -> Result<(), Error> {
    println!("Teleport Client {}", VERSION);

    let files = get_file_list(&opt);

    if files.is_empty() {
        println!(" => No files to send. (Did you mean to add '-r'?)");
        return Ok(());
    }

    // For each filepath in the input vector...
    for (num, item) in files.iter().enumerate() {
        let filepath = item;
        let mut filename = filepath.clone().to_string();

        // Validate file
        let file = match File::open(&filepath) {
            Ok(f) => f,
            Err(s) => {
                println!("Error opening file: {}", filepath);
                return Err(s);
            }
        };

        let thread_file = filepath.clone().to_string();
        let handle = thread::spawn(move || utils::calc_file_hash(thread_file).unwrap());

        // Remove '/' root if exists
        if opt.recursive && filepath.starts_with('/') {
            filename.remove(0);
        } else if !opt.recursive {
            filename = Path::new(item)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
        }

        let meta = file.metadata()?;
        let header = TeleportInit {
            protocol: PROTOCOL.to_string(),
            version: VERSION.to_string(),
            filenum: (num + 1) as u64,
            totalfiles: files.len() as u64,
            filesize: meta.len(),
            filename: filename.to_string(),
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
        stream.write(&header.serialize())?;

        // Receive response from server
        let recv = match recv_ack(&stream) {
            Some(t) => t,
            None => {
                println!("Receive TeleportInitAck timed out");
                return Ok(());
            }
        };

        // Validate response
        match &recv.ack {
            TeleportInitStatus::Overwrite => {
                println!("The server is overwriting the file: {}", &header.filename)
            }
            TeleportInitStatus::NoOverwrite => {
                println!(
                    "The server refused to overwrite the file: {}",
                    &header.filename
                );
                continue;
            }
            TeleportInitStatus::NoPermission => {
                println!(
                    "The server does not have permission to write to this file: {}",
                    &header.filename
                );
                continue;
            }
            TeleportInitStatus::NoSpace => {
                println!(
                    "The server has no space available to write the file: {}",
                    &header.filename
                );
                continue;
            }
            TeleportInitStatus::WrongVersion => {
                println!(
                    "Error: Version mismatch! Server: {} Us: {}",
                    recv.version, VERSION
                );
                break;
            }
            _ => (),
        };

        let csum_recv = recv.delta.as_ref().map(|r| r.csum);
        let checksum = Some(handle.join().expect("calc_file_hash panicked"));

        if checksum == csum_recv {
            // File matches hash
            send_delta_complete(stream, file)?;
        } else {
            // Send file data
            send(stream, file, header, recv.delta)?;
        }

        println!(" done!");
    }
    Ok(())
}

fn recv_ack(mut stream: &TcpStream) -> Option<TeleportInitAck> {
    let mut buf: [u8; 4096 * 3] = [0; 4096 * 3];

    // Receive ACK that the server is ready for data
    let len = match stream.read(&mut buf) {
        Ok(l) => l,
        Err(_) => return None,
    };

    let fix = &buf[..len];
    let mut resp = TeleportInitAck::new(TeleportInitStatus::WrongVersion);
    if let Err(e) = resp.deserialize(fix.to_vec()) {
        println!("{:?}", e);
        return None;
    };

    Some(resp)
}

fn send_delta_complete(mut stream: TcpStream, file: File) -> Result<(), Error> {
    let meta = file.metadata()?;

    let chunk = TeleportData {
        length: 0,
        offset: meta.len() as u64,
        data: Vec::<u8>::new(),
    };

    // Send the data chunk
    stream.write_all(&chunk.serialize())?;
    stream.flush()?;

    Ok(())
}

/// Send function receives the ACK for data and sends the file data
fn send(
    mut stream: TcpStream,
    mut file: File,
    header: TeleportInit,
    delta: Option<TeleportDelta>,
) -> Result<(), Error> {
    let mut buf = Vec::<u8>::new();
    let mut hash_list = Vec::<Hash>::new();
    let mut hasher = blake3::Hasher::new();
    match delta {
        Some(d) => {
            buf.resize(d.delta_size as usize, 0);
            hash_list = d.delta_csum;
        }
        None => buf.resize(4096, 0),
    }

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

        // Check if hash matches, if so: skip chunk
        let index = sent / buf.len();
        if !hash_list.is_empty() && index < hash_list.len() {
            hasher.update(&buf);
            if (index == hash_list.len() - 1) && sent + len == header.filesize as usize {
                send_delta_complete(stream, file)?;
                sent += len;
                print_updates(sent as f64, &header);
                break;
            }
            if hash_list[index] == hasher.finalize() {
                sent += len;
                continue;
            }
            hasher.reset();
        }

        let data = &buf[..len];
        let chunk = TeleportData {
            length: len as u32,
            offset: sent as u64,
            data: data.to_vec(),
        };

        // Send the data chunk
        match stream.write_all(&chunk.serialize()) {
            Ok(_) => {}
            Err(s) => return Err(s),
        };
        match stream.flush() {
            Ok(_) => {}
            Err(s) => return Err(s),
        };

        sent += len;
        print_updates(sent as f64, &header);
    }

    Ok(())
}
