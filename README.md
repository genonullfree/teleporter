# Teleport

Teleport is a small utility in the vein of netcat to send files quickly from point A to point B. It is more convenient than netcat in that you don't have to run a separate command with file redirection for each file you wish to transfer.

Teleport lets you pass the destination and a list of files you wish to send and it will create those files with the proper filenames on the receiving end. Each Teleport binary can act as a client or a server so there's no need to move multiple software packages around.

# Usage
```
USAGE:
    teleport [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -d, --dest <dest>         Destination teleport IP address [default: 127.0.0.1]
    -i, --input <input>...    List of filepaths to files that will be teleported [default: ""]
    -p, --port <port>         Destination teleport Port, or Port to listen on [default: 9001]
```

To start a teleport in server (receiving) mode, just run:
```
./teleport
```
or
```
cargo run
```
Teleport will default to listening on `0.0.0.0:9001` for incoming connections.

To start a teleport in client (sending) mode, run:
```
./teleport -d <destination IP> -i <file> [[file2] [file3] ...]
```

Teleport will only transfer files with their name information. Any file path information will be lost. All the received files will be written out in the CWD where the server side was started.

# WIP Disclaimer

Teleport is currently a work in progress. There is no error checking or anything like that (nc doesn't either :P). Use at your own risk.
