# Teleport

Teleport is a small utility in the vein of netcat to send files quickly from point A to point B. It is more convenient than netcat in that you don't have to run a separate command with file redirection for each file you wish to transfer.

Teleport lets you pass the destination and a list of files you wish to send and it will create those files with the proper filenames on the receiving end. Each Teleport binary can act as a client or a server so there's no need to move multiple software packages around.

Teleport can recursively copy files as well, just pass a directory name and it will copy files all the way down.

Teleport now does delta file transfers using the Blake3 hashing algorithm for files being overwritten that are larger than 1Mb. Testing has shown this to increase speedup for large files by about half.

# Usage
```
Teleport is a simple application for sending files from Point A to Point B

USAGE:
    teleport [FLAGS] [OPTIONS]

FLAGS:
    -h, --help         Prints help information
    -o, --overwrite    Overwrite remote file
    -r, --recursive    Recurse into directories on send
    -V, --version      Prints version information

OPTIONS:
    -d, --dest <dest>         Destination teleport IP address [default: 127.0.0.1]
    -i, --input <input>...    List of filepaths to files that will be teleported [default: ]
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

Teleport will transfer files with their name information as well as their file permissions. Any file path information will be lost. All the received files will be written out in the CWD where the server side was started.

# Example output

## Server (receiving from 2 different clients)

```
$ target/debug/teleport
Teleport Server listening for connections on 0.0.0.0:9001
Receiving: ["testfile", "otherfile", "testfile2", "testfile3"] => Received file: testfile2 from: 127.0.0.1:41330
Receiving: ["testfile", "otherfile", "testfile3", "testfile4"] => Received file: testfile3 from: 127.0.0.1:41332
Receiving: ["testfile", "otherfile", "testfile4"] => Received file: testfile from: 127.0.0.1:41326
Receiving: ["otherfile", "testfile4", "testfile5"] => Received file: testfile5 from: 127.0.0.1:41336
Receiving: ["otherfile", "testfile4", "testfileB"] => Received file: testfile4 from: 127.0.0.1:41334
Receiving: ["otherfile", "testfileB"] => Received file: testfileB from: 127.0.0.1:41340
Receiving: ["otherfile", "testfileC"]
```

## Client (sending)

```
$ target/debug/teleport -i ./test/testfile ./test/testfile2 ./test/testfile3 ./test/testfile4
Teleport Client
Sending file 1/4: "testfile"
 =>    2.000G of    2.000G (100.00%) done!
Sending file 2/4: "testfile2"
 =>    4.000M of    4.000M (100.00%) done!
Sending file 3/4: "testfile3"
 =>    4.000M of    4.000M (100.00%) done!
Sending file 4/4: "testfile4"
 =>   20.000M of   20.000M (100.00%) done!

```

# WIP Disclaimer

Teleport is currently a work in progress. Use at your own risk.
