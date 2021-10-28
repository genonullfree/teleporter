# Teleport Protocol Documentation

The Teleport protocol is a semi-simple protocol that can transfer a file from one location to another
quickly, while also transmitting metadata. The protocol involves two endpoints, the client and the
server. The server binds and listens for incoming connections on a TCP socket (default TCP port 9001).
The client will connect to the server at the specified port and begin transferring the metadata and
file data for each file in the command line arguments provided.

## Protocol

The packet that initiates the connection is the `TeleportInit` packet, defined as follows:
```rust
pub struct TeleportInit {
    protocol: String,
    version: String,
    filename: String,
    filenum: u64,
    totalfiles: u64,
    filesize: u64,
    chmod: u32,
    overwrite: bool,
}
```

The `protocol` string is always `TELEPORT`. The `version` string is whatever the current version of
the client is, for example: `0.4.6`. The next string is the `filename` of the file that is being
sent. The strings are all NULL-terminated. The `filenum` field is a unsigned 64bit integer that is
the file number of the current file in a batch of files and starts counting at `1`. `totalfile` is
the number of files that will be sent together in the current batch and also starts counting at `1`.
`filesize` is the size of the current file in bytes. `chmod` is the current file permissions to be
applied to the file when it is received on the server side. `overwrite` is a boolean that indicates
to the server if this file is to overwrite an existing file on the server, if it exists.

The `TeleportInit` file is responded to with a `TeleportAck`, which has the following properties:
```rust
pub struct TeleportInitAck {
    ack: TeleportInitStatus,
    version: String,
    delta: Option<TeleportDelta>,
}
```

The values of `ack` are of the enumerated type `TeleportInitStatus`, which are described below. The
`version` string is NULL-terminated and is the current version of the server. Only the `major` and
`minor` versions of the version string must match; the point release must not include protocol
breaking changes. The optional `delta` field is included last and is described after
`TeleportInitStatus`.

```rust
pub enum TeleportInitStatus {
    Proceed,      // Success
    Overwrite,    // Success, delta overwrite
    NoOverwrite,  // Error
    NoSpace,      // Error
    NoPermission, // Error
    WrongVersion, // Error
}
```
The values `Proceed` and `Overwrite` give the feedback to the client that it is ready to proceed with
the file transfer. `Overwrite` specifically indicates that there is a file on the server side that
will be overwritten. All the other values are specific error scenarios that cause the client to not
proceed with the file transfer.

```rust
pub struct TeleportDelta {
    size: u64,
    delta_size: u64,
    csum: Hash,
    delta_csum: Vec<Hash>,
}
```
The `TeleportDelta` option is included when the client receives an `Overwrite` `ack` value. This
struct is constructed to inform to the client what chunks need to be sent to the server to perform
a delta file transfer. Delta file transfers can save valuable time by only transferring parts of
the file that are different. The `size` value is sent to indicate what the size of the file that
exists on the server is, to be compared with the file size on the client. The `delta_size` relates
how large of blocks of data are to be used for the delta chunks. `csum` is a Blake3 hash value of
the entire file on the server, and `delta_csum` is a vector of Blake3 hash values for each chunk
of length `delta_size` in the file. The Blake3 hash values are 32 bytes in length, so the length
of the vector must be evenly divisible by 32.

Once the server replies back to the client with a `Proceed` or `Overwrite` `TeleportInitAck` packet,
the client will begin sending data. If the server sent an `Overwrite` ack, then the client will
read in `delta_size` chunks and hash them with Blake3. If the hash value matches for the vector
value in `delta_csum` then it will not send the chunk and will iterate to the next one. When a hash
value does not match, indicating a chunk was changed, or when the server is not overwriting a file,
the client will chunk up the file to be transferred and send it to the server using `TeleportData`
structs, defined as:
```rust
pub struct TeleportData {
    length: u32,
    offset: u64,
    data: Vec<u8>,
}
```

The `length` value is the size of the `data` vector in bytes. The `offset` value is the location in
the file to begin writing the chunk to. The `data` vector is a vector of unsigned bytes of data that
are the file data.

Every `TeleportData` packet is responded to with a `TeleportDataAck` packet, defined as:
```rust
pub struct TeleportDataAck {
    ack: TeleportDataStatus,
}
```

`TeleportDataStatus` is an enumerated value with the values of:
```rust
pub enum TeleportDataStatus {
    Success,
    Error,
}
```

Once the file is completely transferred the TCP connection is closed. If there is another file to
transfer from the client, a new TCP connection is made.
