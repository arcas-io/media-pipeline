use crate::create_pipeline;
use crate::error::Result;
use crate::main_loop::main_loop_simple;
use bytes::Bytes;
use std::sync::mpsc::{channel, Receiver, Sender};

fn pipeline() -> Result<gstreamer::Pipeline> {
    let launch = "videotestsrc ! video/x-raw,format=I420,framerate=30/1,width=1280,height=720 ! x264enc tune=zerolatency ! rtph264pay ! udpsink port=5000 host=127.0.0.1";
    let pipeline = create_pipeline(launch)?;

    Ok(pipeline)
}

pub fn start() -> (Sender<Bytes>, Receiver<Bytes>) {
    let (send, recv) = channel::<Bytes>();

    std::thread::spawn(|| {
        match pipeline().and_then(main_loop_simple) {
            Ok(r) => r,
            Err(e) => log::error!("Error! {}", e),
        };
    });

    (send, recv)
}

#[cfg(test)]
mod tests {

    use super::*;

    // ignore test, this is used for other tests in dev mode
    #[test]
    #[cfg_attr(not(feature = "test_udp_server"), ignore)]
    fn it_serves_rtp_via_udp() {
        env_logger::try_init().ok();

        let (_tx, rx) = start();

        while let Ok(_bytes) = rx.recv() {}
    }
}
