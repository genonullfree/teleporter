use crate::errors::TeleportError;
use crate::teleport::{TeleportAction, TeleportFeatures, TeleportStatus};
use crate::teleport::{TeleportData, TeleportDelta, TeleportEnc, TeleportInit, TeleportInitAck};
use crate::SendOpt;
use crate::VERSION;
use crate::{crypto, utils};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;

#[derive(Debug)]
struct Replace {
    orig: Vec<String>,
    new: Vec<String>,
}

fn get_file_list(opt: &SendOpt) -> Vec<String> {
    let mut files = Vec::<String>::new();

    // Iterate over each item in list
    for item in opt.input.iter() {
        if opt.recursive && item.is_dir() {
            // Recurse into directories
            let mut tmp = match scope_dir(item) {
                Ok(t) => t,
                Err(_) => {
                    println!("Error: Cannot read item: {item:?}");
                    continue;
                }
            };
            // Append any files located
            files.append(&mut tmp);
        } else if item.exists() && item.is_file() {
            // Append the file
            files.push(
                item.to_str()
                    .expect("Fatal error converting item to str")
                    .to_string(),
            );
        }
    }

    files
}

fn scope_dir(dir: &Path) -> Result<Vec<String>, TeleportError> {
    let path = Path::new(&dir);
    let mut files = Vec::<String>::new();

    // Iterate over each item in directory
    for entry in path.read_dir()? {
        if entry.as_ref().unwrap().file_type().unwrap().is_dir() {
            // Skip current directory
            if entry.as_ref().unwrap().path() == *dir {
                continue;
            }

            // Recurse into subdirectories
            let mut tmp = match scope_dir(&entry.as_ref().unwrap().path()) {
                Ok(t) => t,
                Err(_) => {
                    println!("Error: Cannot read dir: {entry:?}");
                    continue;
                }
            };
            // Append any files located
            files.append(&mut tmp);
        } else if entry.as_ref().unwrap().file_type().unwrap().is_file() {
            // Append the file
            files.push(entry.unwrap().path().to_str().unwrap().to_string());
        }
    }

    Ok(files)
}

fn find_replacements(opt: &mut SendOpt) -> Replace {
    let mut rep = Replace {
        orig: Vec::<String>::new(),
        new: Vec::<String>::new(),
    };

    let mut orig: String;
    let mut new: String;
    let mut poppers = Vec::<usize>::new();

    // Iterate over the input list
    for (idx, item) in opt.input.iter().enumerate() {
        // If it is a filename, no rename is happening
        if File::open(item).is_ok() {
            continue;
        }

        let path = item.to_str().unwrap();

        // If the path name contains ':'
        if path.contains(':') {
            // Split on ':' and use the first as original name
            // and the second as the new name
            let mut split = path.split(':');
            orig = split.next().unwrap().to_string();
            new = split.next().unwrap().to_string();

            // If the original name can be opened, proceed with rename
            if File::open(&orig).is_ok() {
                rep.orig.push(orig.clone());
                rep.new.push(new.clone());
                poppers.push(idx);
            }
        }
    }

    // For every replacement being made
    while !poppers.is_empty() {
        // Get the index of the string to be replaced
        let idx = poppers.pop().unwrap();
        // Remove the string from the input list
        opt.input.remove(idx);
        // Insert the original file name to be used
        opt.input
            .insert(idx, PathBuf::from(&rep.orig[poppers.len()]));
    }

    // Return the list of replacement names
    rep
}

fn connect_to_client(
    ip_addrs: std::vec::IntoIter<std::net::SocketAddr>,
) -> Result<TcpStream, TeleportError> {
    for addr in ip_addrs {
        match TcpStream::connect(addr) {
            Ok(s) => return Ok(s),
            Err(_) => {
                continue;
            }
        };
    }

    Err(TeleportError::InvalidDest)
}

/// Client function sends filename and file data for each filepath
pub fn run(mut opt: SendOpt) -> Result<(), TeleportError> {
    print!("Teleporter Client {VERSION} => ");
    let start_time = Instant::now();
    let mut sent = 0;
    let mut skip = 0;

    // Generate a list of replacement names and fix up the input list
    let rep = find_replacements(&mut opt);

    // Generate the file list
    let files = get_file_list(&opt);

    // If file list is empty, exit
    if files.is_empty() {
        println!(" => No files to send. (Did you mean to add '-r'?)");
        return Ok(());
    }

    // For each filepath in the input vector...
    for (num, item) in files.iter().enumerate() {
        let file_time = Instant::now();

        let mut enc: Option<TeleportEnc> = None;

        let filepath = item;
        let mut filename = filepath.clone().to_string();

        // Locate and replace the filename of the transfer file, if renamed
        for (idx, item) in rep.orig.iter().enumerate() {
            if item.contains(&filepath.to_string()) {
                filename = rep.new[idx].clone();
            }
        }

        // Validate file
        let file = match File::open(filepath) {
            Ok(f) => f,
            Err(s) => {
                println!("Error opening file: {filepath}");
                return Err(TeleportError::Io(s));
            }
        };

        let thread_file = File::open(filepath)?;
        // Skip if opt.no_delta present, otherwise calculate the delta hash of the file
        let handle = match opt.overwrite && !opt.no_delta {
            true => Some(thread::spawn(move || {
                utils::calc_delta_hash(&thread_file).unwrap()
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

        // Populate features
        let meta = file.metadata()?;
        let mut header = TeleportInit::new(TeleportFeatures::NewFile);
        let mut features: u32 = 0;

        // Add delta flag by default
        if !opt.no_delta {
            TeleportFeatures::Delta.add_u32(&mut features);
        }

        // Add overwrite flag if enabled
        if opt.overwrite {
            TeleportFeatures::Overwrite.add_u32(&mut features);
        }

        // Add backup flag if enabled
        if opt.backup {
            TeleportFeatures::Backup.add_u32(&mut features);
        }

        // Add rename flag if enabled
        if opt.filename_append {
            TeleportFeatures::Rename.add_u32(&mut features);
        }
        header.features = features;
        header.chmod = meta.permissions().mode();
        header.filesize = meta.len();
        header.filename = filename.as_bytes().to_vec();

        // Connect to server
        let addr = match format!("{}:{}", opt.dest, opt.port).to_socket_addrs() {
            Ok(a) => a,
            Err(_) => {
                return Err(TeleportError::InvalidDest);
            }
        };
        let mut stream = connect_to_client(addr)?;

        // If encrypt is enabled
        if opt.encrypt {
            // Generate EC keypair
            let mut ctx = TeleportEnc::new();
            let privkey = crypto::genkey(&mut ctx);
            // Send pubkey
            utils::send_packet(&mut stream, TeleportAction::Ecdh, &None, ctx.serialize())?;
            // Receive remote pubkey and generate session secret
            let packet = utils::recv_packet(&mut stream, &None)?;
            if packet.action == TeleportAction::EcdhAck as u8 {
                ctx.deserialize(&packet.data)?;
                ctx.calc_secret(privkey);
                enc = Some(ctx);
            }
        }

        // Send header first
        utils::send_packet(&mut stream, TeleportAction::Init, &enc, header.serialize()?)?;

        // Receive response from server
        let packet = utils::recv_packet(&mut stream, &enc)?;
        let mut recv = TeleportInitAck::new(TeleportStatus::Proceed);
        recv.deserialize(&packet.data)?;

        if num == 0 {
            println!(
                "Server {}.{}.{}",
                recv.version[0], recv.version[1], recv.version[2]
            );
        }

        // Validate response
        match recv.status.try_into()? {
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
                    "Version mismatch! Server: {:?} Us: {}",
                    recv.version, VERSION
                );
                break;
            }
            TeleportStatus::RequiresEncryption => {
                println!("The server requires encryption");
                break;
            }
            TeleportStatus::EncryptionError => {
                println!("Error initializing encryption handshake");
                break;
            }
            _ => (),
        };

        // If TeleportDelta was received, else None
        let csum_recv = recv.delta.as_ref().map(|r| r.hash);
        let mut file_delta: Option<TeleportDelta> = None;
        if TeleportFeatures::Overwrite.check(&recv.features) {
            file_delta = handle.map(|s| s.join().expect("calc_file_hash panicked"));
        }

        println!("Sending file {}/{}: {}", num + 1, files.len(), &filename);

        if csum_recv.is_some()
            && file_delta.is_some()
            && file_delta.as_ref().unwrap().hash == csum_recv.unwrap()
        {
            // File matches hash
            send_data_complete(stream, &enc, header.filesize)?;
            skip += 1;
        } else {
            // Send file data
            send(stream, file, &header, &enc, recv.delta, file_delta)?;
            sent += 1;
        }

        // Print file transfer statistics
        let duration = file_time.elapsed();
        let speed = (header.filesize as f64 * 8.0) / duration.as_secs() as f64 / 1024.0 / 1024.0;
        println!(" done! Time: {duration:.2?} Speed: {speed:.3} Mbps");
    }
    let total_time = start_time.elapsed();
    println!(
        "Teleported {}/{}/{} Sent/Same/Total in {:.2?}",
        sent,
        skip,
        sent + skip,
        total_time
    );
    Ok(())
}

fn send_data_complete(
    mut stream: TcpStream,
    enc: &Option<TeleportEnc>,
    filesize: u64,
) -> Result<(), TeleportError> {
    let mut chunk = TeleportData {
        offset: filesize,
        data_len: 0,
        data: Vec::<u8>::new(),
    };

    // Send the data chunk
    utils::send_packet(&mut stream, TeleportAction::Data, enc, chunk.serialize()?)?;

    Ok(())
}

/// Send function receives the ACK for data and sends the file data
fn send(
    mut stream: TcpStream,
    mut file: File,
    header: &TeleportInit,
    enc: &Option<TeleportEnc>,
    delta: Option<TeleportDelta>,
    file_delta: Option<TeleportDelta>,
) -> Result<(), TeleportError> {
    let mut buf = Vec::<u8>::new();
    let meta = file.metadata()?;

    // Set transfer chunk size to delta chunk size, or default to 4096
    match delta {
        Some(ref d) => buf.resize(d.chunk_size as usize, 0),
        None => buf.resize(4096, 0),
    }

    // If present, get the lengths of the delta hash arrays
    let compare_delta = delta.is_some() && file_delta.is_some();
    let delta_len = if delta.is_some() {
        delta.as_ref().unwrap().chunk_hash.len()
    } else {
        0
    };
    let file_delta_len = if file_delta.is_some() {
        file_delta.as_ref().unwrap().chunk_hash.len()
    } else {
        0
    };

    // Send file data
    let mut sent = 0;
    loop {
        // Check if hash matches, if so: skip chunk
        let index = sent / buf.len();
        if compare_delta
            && index < delta_len
            && index < file_delta_len
            && delta.as_ref().unwrap().chunk_hash[index]
                == file_delta.as_ref().unwrap().chunk_hash[index]
        {
            sent += buf.len();
            continue;
        }

        file.seek(SeekFrom::Start(sent as u64))?;
        // Read a chunk of the file
        let len = match file.read(&mut buf) {
            Ok(l) => l,
            Err(s) => return Err(TeleportError::Io(s)),
        };

        // If a length of 0 was read, we're done sending
        if len == 0 {
            break;
        }

        let data = &buf[..len];
        let mut chunk = TeleportData {
            offset: sent as u64,
            data_len: len as u32,
            data: data.to_vec(),
        };

        // Send the data chunk
        utils::send_packet(&mut stream, TeleportAction::Data, enc, chunk.serialize()?)?;

        sent += len;
        utils::print_updates(sent as f64, header);
    }

    send_data_complete(stream, enc, meta.len())?;

    Ok(())
}
