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

# Example output

## Server (receiving)

```
$ target/debug/teleport
Server mode, listening for connections
Receiving file 1/4: "testfile" (from 127.0.0.1:52366)
 =>    2.000G of    2.000G (100.00%) done!
Receiving file 2/4: "testfile2" (from 127.0.0.1:52368)
 =>    4.000M of    4.000M (100.00%) done!
Receiving file 3/4: "testfile3" (from 127.0.0.1:52370)
 =>    4.000M of    4.000M (100.00%) done!
Receiving file 4/4: "testfile4" (from 127.0.0.1:52372)
 =>   20.000M of   20.000M (100.00%) done!
```

## Client (sending)

```
$ target/debug/teleport -i ./test/testfile ./test/testfile2 ./test/testfile3 ./test/testfile4
Client mode
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

Teleport is currently a work in progress. There is no error checking or anything like that (nc doesn't either :P). Use at your own risk.
