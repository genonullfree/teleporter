# Teleporter

Teleporter is a small utility in the vein of netcat to send files quickly from point A to point B. It is more convenient than netcat in that you don't have to run a separate command with file redirection for each file you wish to transfer.

Teleporter lets you pass the destination and a list of files you wish to send and it will create those files with the proper filenames on the receiving end. Each Teleporter binary can act as a client or a server so there's no need to move multiple software packages around.

Teleporter can recursively copy, overwrite, rename, and keep a backup of the destination file.

Teleporter now does delta file transfers using the xxHash3 hashing algorithm for files being overwritten, hashing the entire file as well as splitting the file into a number of smaller chunks.

The protocol Teleporter implements to transfer files is called Teleport and is defined in ``PROTOCOL.md``.

# Usage
```
Teleporter is a simple application for sending files from Point A to Point B

USAGE:
    teleporter [FLAGS] [OPTIONS]

FLAGS:
        --allow-dangerous-filepath    Allow absolute and relative file paths for transfers (server only) [WARNING:
                                      potentially dangerous option, use at your own risk!]
    -b, --backup                      Backup the destination file to a ".bak" extension if it exists and is being
                                      overwritten (consecutive runs will replace the *.bak file)
    -e, --encrypt                     Encrypt the file transfer using ECDH key-exchange and random keys
    -f, --filename-append             If the destination file exists, append a ".1" (or next available number) to the
                                      filename instead of overwriting
    -h, --help                        Prints help information
    -k, --keep-path                   Keep path info (recreate directory path on remote server)
    -n, --no-delta                    Disable delta transfer (overwrite will transfer entire file)
    -o, --overwrite                   Overwrite remote file
    -r, --recursive                   Recurse into directories on send
    -V, --version                     Prints version information

OPTIONS:
    -d, --dest <dest>         Destination teleporter IP address [default: 127.0.0.1]
    -i, --input <input>...    List of filepaths to files that will be teleported [default: ]
    -p, --port <port>         Destination teleporter Port, or Port to listen on [default: 9001]
```

To start a teleporter in server (receiving) mode, just run:
```
teleporter
```
or
```
cargo run
```
Teleporter will default to listening on `0.0.0.0:9001` for incoming connections.

To start a teleporter in client (sending) mode, run:
```
teleporter [-d <destination IP>] -i <file> [[file2] [file3] ...]
```

Teleporter will transfer files with their name information as well as their file permissions. Any file path information will be lost unless the `-k` option is enabled. All the received files will be written out in the CWD where the server side was started unless the server was started with the `--allow-dangerous-filepath` option. When overwriting a file with the `-o` option, additional modifiers can be used, such as `-b` to make a backup of the original file, or `-n` to disable delta file transfers and always overwrite the entire file. 

## Rename / Copy-To

Teleporter can now set remote file locations, or file renaming, via the `:` operator. Similar to how `Docker` allows quick mounting of directory locations, Teleporter will first attempt to open a file by the full given path, if that file does not exist, it will see if there are any colons (`:`) in the filename. If present, it will split the filepath and attempt to open on the first portion of the name. If that succeeds, Teleporter assumes this is a file rename / copy-to. Teleporter will also need the `-k` option, to keep filepath information. Otherwise only the file name will be changed.

For example, given the following command:
```bash
./teleporter -i ~/Downloads/ubuntu-20.04.3-live-server-arm64.iso:/tmp/ubuntu.iso -k
```
(and assuming the server was started with `--allow-dangerous-filepath`), Teleporter will first attempt to open `~/Downloads/ubuntu-20.04.3-live-server-arm64.iso:/tmp/ubuntu.iso`, if that fails, it will attempt to split the path on `:` and open `~/Downloads/ubuntu-20.04.3-live-server-arm64.iso`. If that succeeds, then it knows it is a rename / copy-to operation and will set the destination filepath to be the second part of the string: `/tmp/ubuntu.iso`. On the server, it will only receive the file for `/tmp/ubuntu.iso`. If the `-k` argument was ommitted, the server would just receive the original file renamed as `ubuntu.iso`.

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
