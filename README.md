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
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

let filename = "it_records_rtp_via_udp.mp4";
let (inbound_sender, inbound_receiver) = channel::<Command>();
let (outbound_sender, outbound_receiver) = channel::<Command>();

// record the video in a separate thread
std::thread::spawn(move || {
    record(filename, inbound_receiver, outbound_sender).unwrap();
});

// record for 2 seconds
sleep(Duration::from_millis(2000));

// stop recording
inbound_sender.send(Command::Stop).unwrap();

// listen for commands
while let Ok(command) = outbound_receiver.recv() {
    match command {
        Command::Stopped => {
            log::info!("received Command::Stopped");
        }
        _ => {}
    }
}
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