# Media Pipeline

An abstraction over the [rust bindings](https://github.com/sdroege/gstreamer-rs) of GStreamer.

## Usage

### Read RTP from a Stream

```rust
use media_pipeline::rtp_stream::start;

let (tx, rx) = start();

while let Ok(bytes) = rx.recv() {
    // do something with bytes
}
```

### Record MP4 from RTP packets in a Stream

```rust
use media_pipeline::{main_loop::Command, rtp_stream_record::record};
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

let filename = "test/output/it_records_rtp_via_stream.mp4";
let (inbound_sender, inbound_receiver) = channel::<Command>();
let (outbound_sender, outbound_receiver) = channel::<Command>();

// start the rtp stream
let (_tx, rx) = start();

// record the video in a separate thread
std::thread::spawn(move || {
    record(filename, rx, inbound_receiver, outbound_sender)
        .map_err(|error| log::error!("Error recording: {:?}", error));
});

// record for 4 seconds
sleep(Duration::from_millis(4000));

// stop recording
inbound_sender.send(Command::Stop)
    .map_err(|error| log::error!("Error sending Command:Stop to the main loop: {:?}", error));

// listen for commands
while let Ok(command) = outbound_receiver.recv() {
    match command {
        Command::Stopped => {
            println!("received Command::Stopped");
        }
        _ => {}
    }
}
```

### Record MP4 from RTP packets on a UDP Port

```rust
use media_pipeline::{main_loop::Command, rtp_udp_client_record::record};
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

let filename = "it_records_rtp_via_udp.mp4";
let (inbound_sender, inbound_receiver) = channel::<Command>();
let (outbound_sender, outbound_receiver) = channel::<Command>();

// at this point, a source should be sending RTP packets to a UDP port

// record the video in a separate thread
std::thread::spawn(move || {
    record("5000", filename, inbound_receiver, outbound_sender)
        .map_err(|error| log::error!("Error recording: {:?}", error));
});

// record for 2 seconds
sleep(Duration::from_millis(2000));

// stop recording
inbound_sender.send(Command::Stop)
    .map_err(|error| log::error!("Error sending Command:Stop to the main loop: {:?}", error));

// listen for commands
while let Ok(command) = outbound_receiver.recv() {
    match command {
        Command::Stopped => {
            println!("received Command::Stopped");
        }
        _ => {}
    }
}
```

### Invoking a Test UDP Server

```shell
cargo test it_serves_rtp_via_udp --features "test_udp_server"
```