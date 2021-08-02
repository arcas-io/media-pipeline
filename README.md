# Media Pipeline

An abstraction over the rust bindings of GStreamer.

## Usage

### Read RTP from a Stream

```rust
use media_pipeline::rtp_stream::start;

let (tx, rx) = start();

while let Ok(bytes) = rx.recv() {
    // do something with bytes
}
```

### Record mp4 from a UDP Port

```rust
use media_pipeline::rtp_udp_client_record::record;
use std::sync::mpsc::channel;

let filename = "test/output/it_records_rtp_via_udp.mp4";
let (tx, rx) = channel::<Command>();
let sender = tx.clone();

// record in a separate thread
std::thread::spawn(move || {
    record(filename, sender, rx).unwrap();
});

// record for 2 seconds
sleep(Duration::from_millis(2000));

// stop recording
tx.send(Command::Stop).unwrap();
```

## Running PoC Binaries

### Audio Only

```shell
cargo run --bin audio
```

### Video Only

```shell
cargo run --bin video
```

### Audio and Video

```shell
cargo run --bin audiovideo
```