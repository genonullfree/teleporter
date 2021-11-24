use crate::teleport::*;
use crate::teleport::{TeleportAction, TeleportFeatures, TeleportStatus};
use crate::teleport::{TeleportInit, TeleportInitAck};
use crate::utils::print_updates;
use crate::*;
use ahash::AHasher;
use std::hash::Hasher;
use std::path::Path;

#[derive(Debug)]
struct Replace {
    orig: Vec<String>,
    new: Vec<String>,
}

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
        } else if item.exists() && item.is_file() {
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

fn find_replacements(opt: &mut Opt) -> Replace {
    let mut rep = Replace {
        orig: Vec::<String>::new(),
        new: Vec::<String>::new(),
    };

    let mut orig: String;
    let mut new: String;
    let mut poppers = Vec::<usize>::new();

    for (idx, item) in opt.input.iter().enumerate() {
        if File::open(&item).is_ok() {
            continue;
        }

        let path = item.to_str().unwrap();
        if path.contains(&":") {
            let mut split = path.split(':');
            orig = split.next().unwrap().to_string();
            new = split.next().unwrap().to_string();

            if File::open(&orig).is_ok() {
                rep.orig.push(orig.clone());
                rep.new.push(new.clone());
                poppers.push(idx);
            }
        }
    }

    while !poppers.is_empty() {
        let idx = poppers.pop().unwrap();
        opt.input.remove(idx);
        opt.input
            .insert(idx, PathBuf::from(&rep.orig[poppers.len()]));
    }

    rep
}

/// Client function sends filename and file data for each filepath
pub fn run(mut opt: Opt) -> Result<(), Error> {
    println!("Teleporter Client {}", VERSION);

    let rep = find_replacements(&mut opt);

    let files = get_file_list(&opt);

    if files.is_empty() {
        println!(" => No files to send. (Did you mean to add '-r'?)");
        return Ok(());
    }

    // For each filepath in the input vector...
    for (num, item) in files.iter().enumerate() {
        let start_time = Instant::now();

        let mut enc: Option<TeleportEnc> = None;

        let filepath = item;
        let mut filename = filepath.clone().to_string();
        for (idx, item) in rep.orig.iter().enumerate() {
            if item.contains(&filepath.to_string()) {
                filename = rep.new[idx].clone();
            }
        }

        // Validate file
        let file = match File::open(&filepath) {
            Ok(f) => f,
            Err(s) => {
                println!("Error opening file: {}", filepath);
                return Err(s);
            }
        };

        let thread_file = filepath.clone().to_string();
        let handle = match opt.overwrite && !opt.no_delta {
            true => Some(thread::spawn(move || {
                utils::calc_file_hash(thread_file).unwrap()
            })),
            false => None,
        };

        // Remove all path info if !opt.keep_path
        if !opt.keep_path {
            filename = Path::new(&filename)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
        }

        let meta = file.metadata()?;
        let mut header = TeleportInit::new(TeleportFeatures::NewFile);
        let mut features: u32 = 0;
        if !opt.no_delta {
            features |= TeleportFeatures::Delta as u32;
        }
        if opt.overwrite {
            features |= TeleportFeatures::Overwrite as u32;
        }
        if opt.backup {
            features |= TeleportFeatures::Backup as u32;
        }
        if opt.filename_append {
            features |= TeleportFeatures::Rename as u32;
        }
        header.features = features;
        header.chmod = meta.permissions().mode();
        header.filesize = meta.len();
        header.filename = filename.chars().collect();

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

        if opt.encrypt {
            let mut ctx = TeleportEnc::new();
            utils::send_packet(&mut stream, TeleportAction::Ecdh, &None, ctx.serialize())?;
            let packet = utils::recv_packet(&mut stream, &None)?;
            if packet.action == TeleportAction::EcdhAck as u8 {
                ctx.deserialize(&packet.data)?;
                enc = Some(ctx);
            }
        }

        println!("Sending file {}/{}: {}", num + 1, files.len(), &filename);

        // Send header first
        utils::send_packet(&mut stream, TeleportAction::Init, &enc, header.serialize())?;

        // Receive response from server
        let packet = utils::recv_packet(&mut stream, &enc)?;
        let mut recv = TeleportInitAck::new(TeleportStatus::UnknownAction);
        recv.deserialize(&packet.data)?;

        // Validate response
        match recv.status.try_into().unwrap() {
            TeleportStatus::NoOverwrite => {
                println!("The server refused to overwrite the file: {:?}", &filename);
                continue;
            }
            TeleportStatus::NoPermission => {
                println!(
                    "The server does not have permission to write to this file: {:?}",
                    &filename
                );
                continue;
            }
            TeleportStatus::NoSpace => {
                println!(
                    "The server has no space available to write the file: {:?}",
                    &filename
                );
                continue;
            }
            TeleportStatus::WrongVersion => {
                println!(
                    "Error: Version mismatch! Server: {:?} Us: {}",
                    recv.version, VERSION
                );
                break;
            }
            TeleportStatus::EncryptionError => {
                println!("Error initializing encryption handshake");
                break;
            }
            _ => (),
        };

        let csum_recv = recv.delta.as_ref().map(|r| r.hash);
        let mut hash: Option<u64> = None;
        if utils::check_feature(&recv.features, TeleportFeatures::Overwrite) {
            hash = handle.map(|s| s.join().expect("calc_file_hash panicked"));
        }

        if hash != None && hash == csum_recv {
            // File matches hash
            send_data_complete(stream, &enc, file)?;
        } else {
            // Send file data
            send(stream, file, &header, &enc, recv.delta)?;
        }

        let duration = start_time.elapsed();
        let speed = (header.filesize as f64 * 8.0) / duration.as_secs() as f64 / 1024.0 / 1024.0;
        println!(" done! Time: {:.2?} Speed: {:.3} Mbps", duration, speed);
    }
    Ok(())
}

fn send_data_complete(
    mut stream: TcpStream,
    enc: &Option<TeleportEnc>,
    file: File,
) -> Result<(), Error> {
    let meta = file.metadata()?;

    let mut chunk = TeleportData {
        offset: meta.len() as u64,
        data_len: 0,
        data: Vec::<u8>::new(),
    };

    // Send the data chunk
    utils::send_packet(&mut stream, TeleportAction::Data, enc, chunk.serialize())?;

    Ok(())
}

/// Send function receives the ACK for data and sends the file data
fn send(
    mut stream: TcpStream,
    mut file: File,
    header: &TeleportInit,
    enc: &Option<TeleportEnc>,
    delta: Option<TeleportDelta>,
) -> Result<(), Error> {
    let mut buf = Vec::<u8>::new();
    let mut hash_list = Vec::<u64>::new();
    match delta {
        Some(d) => {
            buf.resize(d.chunk_size as usize, 0);
            hash_list = d.delta_hash;
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
            let mut hasher = AHasher::new_with_keys(0xdead, 0xbeef);
            hasher.write(&buf);
            if hash_list[index] == hasher.finish() {
                sent += len;
                continue;
            }
        }

        let data = &buf[..len];
        let mut chunk = TeleportData {
            offset: sent as u64,
            data_len: len as u32,
            data: data.to_vec(),
        };

        // Send the data chunk
        utils::send_packet(&mut stream, TeleportAction::Data, enc, chunk.serialize())?;

        sent += len;
        print_updates(sent as f64, header);
    }

    send_data_complete(stream, enc, file)?;

    Ok(())
}
