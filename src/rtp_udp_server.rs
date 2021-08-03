use crate::main_loop::main_loop_simple;
use anyhow::Error;
use bytes::Bytes;
use gstreamer::prelude::*;
use std::sync::mpsc::{channel, Receiver, Sender};

fn create_pipeline(_sender: Sender<Bytes>) -> Result<gstreamer::Pipeline, Error> {
    gstreamer::init()?;

    let pipeline = gstreamer::parse_launch(&format!(
        "videotestsrc ! video/x-raw,format=I420,framerate=30/1,width=1280,height=720 ! x264enc tune=zerolatency ! rtph264pay ! udpsink port=5000 host=127.0.0.1"
    ))?
    .downcast::<gstreamer::Pipeline>()
    .expect("Expected a gst::Pipeline");

    Ok(pipeline)
}

pub fn start() -> (Sender<Bytes>, Receiver<Bytes>) {
    let _ = env_logger::try_init();
    let (send, recv) = channel::<Bytes>();
    let sender_outbound = send.clone();

    std::thread::spawn(|| {
        match create_pipeline(send).and_then(main_loop_simple) {
            Ok(r) => r,
            Err(e) => eprintln!("Error! {}", e),
        };
    });

    (sender_outbound, recv)
}

#[cfg(test)]
mod tests {

    use super::*;

    // ignore test, this is used for other tests in dev mode
    #[test]
    #[cfg_attr(not(feature = "test_udp_server"), ignore)]
    fn it_serves_rtp_via_udp() {
        let _ = env_logger::try_init();

        let (_tx, rx) = start();

        while let Ok(_bytes) = rx.recv() {}
    }
}
