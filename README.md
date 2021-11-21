# Teleporter

Teleporter is a small utility in the vein of netcat to send files quickly from point A to point B. It is more convenient than netcat in that you don't have to run a separate command with file redirection for each file you wish to transfer.

Teleporter lets you pass the destination and a list of files you wish to send and it will create those files with the proper filenames on the receiving end. Each Teleporter binary can act as a client or a server so there's no need to move multiple software packages around.

Teleporter can recursively copy files as well, just pass a directory name and it will copy files all the way down.

Teleporter now does delta file transfers using the Blake3 hashing algorithm for files being overwritten.

The protocol Teleporter implements to transfer files is called Teleport.

# Usage
```
Teleporter is a simple application for sending files from Point A to Point B

USAGE:
    teleporter [FLAGS] [OPTIONS]

FLAGS:
    -e, --encrypt      Encrypt the file transfer using ECDH key-exchange and random keys
    -h, --help         Prints help information
    -k, --keep-path    Keep path info (recreate directory path on remote server)
    -n, --no-delta     Disable delta transfer (overwrite will transfer entire file)
    -o, --overwrite    Overwrite remote file
    -r, --recursive    Recurse into directories on send
    -V, --version      Prints version information

OPTIONS:
    -d, --dest <dest>         Destination teleporter IP address [default: 127.0.0.1]
    -i, --input <input>...    List of filepaths to files that will be teleported [default: ]
    -p, --port <port>         Destination teleporter Port, or Port to listen on [default: 9001]
```

To start a teleporter in server (receiving) mode, just run:
```
./teleporter
```
or
```
cargo run
```
Teleporter will default to listening on `0.0.0.0:9001` for incoming connections.

To start a teleporter in client (sending) mode, run:
```
./teleporter -d <destination IP> -i <file> [[file2] [file3] ...]
```

Teleporter will transfer files with their name information as well as their file permissions. Any file path information will be lost. All the received files will be written out in the CWD where the server side was started.

# Installation

If you have Rust and Cargo installed, Teleporter can be quickly compiled and installed by running the following command:
```
cargo install teleporter
```
This will install Teleporter to `~/.cargo/bin/teleporter`, which might need to be added to your shell's `PATH` variable.

# Example output

## Server (receiving from 2 different clients)

```
$ teleporter
Teleporter Server 0.6.0 listening for connections on 0.0.0.0:9001
Receiving: ["archlinux-2021.11.01-x86_64.iso", "ubuntu-20.04.3-live-server-arm64.iso"] => Received file: "archlinux-2021.11.01-x86_64.iso" (from: 127.0.0.1:54708 v[0, 6, 0]) (17.67s @ 398.270 Mbps)
Receiving: ["ubuntu-20.04.3-live-server-arm64.iso", "ArchLinuxARM-aarch64-latest.tar"] => Received file: "ubuntu-20.04.3-live-server-arm64.iso" (from: 127.0.0.1:54709 v[0, 6, 0]) (24.55s @ 390.689 Mbps)
Receiving: ["ArchLinuxARM-aarch64-latest.tar", "laughing_man_by_geno.jpg"] => Received file: "laughing_man_by_geno.jpg" (from: 127.0.0.1:54713 v[0, 6, 0]) (952.46µs @ inf Mbps)
Receiving: ["ArchLinuxARM-aarch64-latest.tar", "unnamed.jpg"] => Received file: "unnamed.jpg" (from: 127.0.0.1:54714 v[0, 6, 0]) (832.04µs @ inf Mbps)
Receiving: ["ArchLinuxARM-aarch64-latest.tar"] => Received file: "ArchLinuxARM-aarch64-latest.tar" (from: 127.0.0.1:54712 v[0, 6, 0]) (27.57s @ 388.182 Mbps)
Receiving: []
```

## Client (sending)

```
$ teleporter -i ~/Downloads/*iso ~/Downloads/ArchLinuxARM-aarch64-latest.tar ~/Downloads/*jpg
Teleporter Client 0.6.0
Sending file 1/5: archlinux-2021.11.01-x86_64.iso
 =>  846.324M of  846.324M (100.00%) done! Time: 17.63s Speed: 398.270 Mbps
Sending file 2/5: ubuntu-20.04.3-live-server-arm64.iso
 =>    1.145G of    1.145G (100.00%) done! Time: 24.51s Speed: 390.689 Mbps
Sending file 3/5: ArchLinuxARM-aarch64-latest.tar
 =>    1.279G of    1.279G (100.00%) done! Time: 27.54s Speed: 388.182 Mbps
Sending file 4/5: laughing_man_by_geno.jpg
 =>   19.230K of   19.230K (100.00%) done! Time: 1.15ms Speed: inf Mbps
Sending file 5/5: unnamed.jpg
 =>   16.374K of   16.374K (100.00%) done! Time: 834.29µs Speed: inf Mbps
```
