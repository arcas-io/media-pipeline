use anyhow::Error;
use bytes::Bytes;
use derive_more::{Display, Error};
use gstreamer::prelude::*;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug, Display, Error)]
#[display(fmt = "Missing element {}", _0)]
struct MissingElement(#[error(not(source))] &'static str);

#[derive(Debug, Display, Error)]
#[display(fmt = "Received error from {}: {} (debug: {:?})", src, error, debug)]
struct ErrorMessage {
    src: String,
    error: String,
    debug: Option<String>,
    source: glib::Error,
}

fn create_pipeline(_sender: Sender<Bytes>) -> Result<gstreamer::Pipeline, Error> {
    gstreamer::init()?;

    let pipeline = gstreamer::parse_launch(&format!(
        "videotestsrc ! video/x-raw,format=I420,framerate=30/1,width=1280,height=720 ! x264enc tune=zerolatency ! rtph264pay ! udpsink port=5000 host=127.0.0.1"
    ))?
    .downcast::<gstreamer::Pipeline>()
    .expect("Expected a gst::Pipeline");

    Ok(pipeline)
}

fn main_loop(pipeline: gstreamer::Pipeline) -> Result<(), Error> {
    pipeline.set_state(gstreamer::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                pipeline.set_state(gstreamer::State::Null)?;
                return Err(ErrorMessage {
                    src: msg
                        .src()
                        .map(|s| String::from(s.path_string()))
                        .unwrap_or_else(|| String::from("None")),
                    error: err.error().to_string(),
                    debug: err.debug(),
                    source: err.error(),
                }
                .into());
            }
            _ => (),
        }
    }

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}

pub fn start() -> (Sender<Bytes>, Receiver<Bytes>) {
    let _ = env_logger::try_init();
    let (send, recv) = channel::<Bytes>();
    let sender_outbound = send.clone();

    std::thread::spawn(|| {
        match create_pipeline(send).and_then(main_loop) {
            Ok(r) => r,
            Err(e) => eprintln!("Error! {}", e),
        };
    });

    (sender_outbound, recv)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_serves_rtp_via_udp() {
        let (tx, rx) = start();
        let mut count = 0;
        let max = 10;

        while let Ok(_bytes) = rx.recv() {
            count += 1;

            if count >= max {
                break;
            }
        }

        assert_eq!(count, max);
    }
}
