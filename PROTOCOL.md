# Teleport Protocol Documentation

The Teleport protocol is a semi-simple protocol that can transfer a file from one location to another
quickly, while also transmitting metadata. The protocol involves two endpoints, the client and the
server. The server binds and listens for incoming connections on a TCP socket (default TCP port 9001).
The client will connect to the server at the specified port and begin transferring the metadata and
file data for each file in the command line arguments provided.

## Protocol

Every packet in the protocol is wrapped in a TeleportHeader, which is defined as:
```rust
pub struct TeleportHeader {
    protocol: u64, // [ 'T', 'E', 'L', 'E', 'P', 'O', 'R', 'T' ]
    data_len: u32,
    pub action: TeleportAction, // as u8
    pub iv: Option<[u8; 12]>,
    pub data: Vec<u8>,
}
```

The protocol field is alwasy `TELEPORT`. The `data_len` is the length of the data field in the packet, which is calculated by adding the `data_len` value with the length of the `protocol`, `data_len`, `action` fields, and optionally `iv` depending on the value in `action` (`8 + 4 + 1` + `12` if `iv.is_some()`). The vector of `data` is deserialized based on what the value of `action` is. `TeleportAction` is defined here:
```rust
pub enum TeleportAction {
    Init = 0x01,
    InitAck = 0x02,
    Ecdh = 0x04,
    EcdhAck = 0x08,
    Data = 0x40,
    Encrypted = 0x80,
}
```

When encryption is enabled, the `action` field is OR'd with the `Encrypted` value, which is how the `TeleportHeader` deserialization knows if the `iv` field is present or not.

For standard unencrypted transfers, the protocol flows like this:
```
Client:                         Server:
TeleportAction::Init ==========>
        <====================== TeleportAction::InitAck
TeleportAction::Data ==========>
TeleportAction::Data ==========>
TeleportAction::Data ==========>
...
```

For encrypted transfers, the protocol flows like this:
```
Client:                                             Server:
TeleportAction::Ecdh ==============================>
        <========================================== TeleportAction::EcdhAck
TeleportAction::Init|TeleportAction::Encrypted ====>
        <========================================== TeleportAction::InitAck|TeleportAction::Encrypted
TeleportAction::Data|TeleportAction::Encrypted ====>
TeleportAction::Data|TeleportAction::Encrypted ====>
TeleportAction::Data|TeleportAction::Encrypted ====>
...
```

The `Ecdh` and `EcdhAck` action packets only contain the Client and Server ECDH public keys, respectively, in the `TeleportHeader`'s `data` field. This allows Teleporter to do an ECDH key exchange and generate a secure secret key. This secret key is used to encrypt the rest of the connection, which will only last for 1 file transfer. Every file transfer renegotiates a new secret key. All the data in the `TeleportHeader` `data` field is encrypted, and the `iv` used is stored in the `iv` field.

The packet that initiates the transfer is the `Init` action packet, defined as follows:
```rust
// Client to server
pub struct TeleportInit {
    pub version: [u16; 3], // [ major, minor, patch ]
    pub features: TeleportFeatures, // as u32
    pub chmod: u32,
    pub filesize: u64,
    pub filename_len: u16,
    pub filename: Vec<char>,
}
```

The `version` value is whatever the current version of the client is, for example: `[0, 4, 6]`. The
`features` value is a bitfield of any requested features that the client supports and would like the
server to support. `chmod` is the current file permissions to be applied to the file when it is
received on the server side. `filesize` is the size of the file to be transferred in bytes. The length
of the filename is stored in `filename_len`, and the vector of characters of the filename is sent in
`filename`.

The current feature set is:
```rust
pub enum TeleportFeatures {
    NewFile = 0x01,
    Delta = 0x02,
    Overwrite = 0x04,
    Backup = 0x08,
    Rename = 0x10,
}
```

`NewFile` is the minimum default feature that should be enabled on any transfer. The `Delta` enables delta
file transfers by hashing the file into chunks and comparing the hash values to calculate the minimum data
that needs to be sent to transfer the file. The `Overwrite` flag allows the Client to send a file and
overwrite a file that already exists on the Server. The `Backup` flag tells the Server to make a backup of
the file if it is being overwritten (saving it to `$filename.bak`). The `Rename` flag tells the server to
save the new file transfer to `$filename.1` instead of overwriting an existing file.


The `TeleportInit` file is responded to with a `TeleportAck`, which has the following properties:
```rust
pub struct TeleportInitAck {
    pub ack: TeleportInitStatus, // as u8
    pub version: [u16; 3],
    pub features: Option<u32>,
    pub delta: Option<TeleportDelta>,
}
```

The values of `ack` are of the enumerated type `TeleportInitStatus` as u8, which are described below. The
`version` array is the current version of the server. Only the `major` and `minor` versions of the version
array must match; the point release must not introduce protocol breaking changes. `features` is an optional
field that is only present if `ack == TeleportInitStatus::Proceed`. The optional `delta` field is included
last if the `Delta` flag is present in the `features` field and is described in detail after
`TeleportInitStatus`. 

```rust
pub enum TeleportInitStatus {
    Proceed,
    NoOverwrite,
    NoSpace,
    NoPermission,
    WrongVersion,
    EncryptionError,
    UnknownAction,
}
```
The value `Proceed` tells the client that it is ready to proceed with the file transfer. All the other
values are specific error scenarios that cause the client to not proceed with the file transfer.

```rust
pub struct TeleportDelta {
    filesize: u64,
    hash: u64,
    chunk_size: u64,
    chunk_hash_len: u16,
    chunk_hash: Vec<u64>,
}
```
The `TeleportDelta` option is included when the client receives an `Overwrite` `feature` value. This
struct is constructed to inform to the client what chunks need to be sent to the server to perform
a delta file transfer. Delta file transfers can save valuable time by only transferring parts of
the file that are different. The `filesize` value is sent to indicate what the size of the file that
exists on the server is, to be compared with the file size on the client. The `chunk_size` relates
how large of blocks of data are to be used for the delta chunks. `hash` is a xxHash3 hash value of
the entire file on the server, and `chunk_hash` is a vector of xxHash3 hash values for each chunk
of length `chunk_size` in the file. The xxHash3 hash values are 8 bytes in length and are stored as u64.

Once the server replies back to the client with a `Proceed` `TeleportInitAck` packet,
the client will begin sending data. If the server sent an `Overwrite` feature back, then the client will
read in `chunk_size` chunks and hash them with xxHash3. If the hash value matches for the vector
value in `chunk_hash` then it will not send the chunk and will iterate to the next one. When a hash
value does not match, indicating a chunk was changed, or when the server is not overwriting a file,
the client will chunk up the file to be transferred and send it to the server using `TeleportData`
structs, defined as:
```rust
pub struct TeleportData {
    offset: u64,
    length: u32,
    data: Vec<u8>,
}
```

The `length` value is the size of the `data` vector in bytes. The `offset` value is the location in
the file to begin writing the chunk to. The `data` vector is a vector of unsigned bytes of data that
are the file data.

Once the file is completely transferred the TCP connection is closed. If there is another file to
transfer from the client, a new TCP connection is made.
