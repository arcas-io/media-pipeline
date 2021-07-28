use anyhow::Error;
use byte_slice_cast::*;
use bytes::Bytes;
use derive_more::{Display, Error};
use gstreamer::element_error;
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

fn create_pipeline(sender: Sender<Bytes>) -> Result<gstreamer::Pipeline, Error> {
    gstreamer::init()?;
    log::info!("create_pipeline 1");

    let pipeline = gstreamer::parse_launch(&format!(
        "udpsrc port=5000 ! application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, payload=(int)96
        ! rtph264depay ! h264parse
        ! tee name=t
        t. ! queue ! mp4mux ! filesink location=xyz.mp4 -e"
    ))?
    .downcast::<gstreamer::Pipeline>()
    .expect("Expected a gst::Pipeline");
    log::info!("create_pipeline 2");

    Ok(pipeline)
}

fn main_loop(pipeline: gstreamer::Pipeline) -> Result<(), Error> {
    log::info!("main loop");
    pipeline.set_state(gstreamer::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        let view = match msg.view() {
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
        };

        log::info!("{:?}", view);
    }

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}

pub fn start() -> (Sender<Bytes>, Receiver<Bytes>) {
    let _ = env_logger::try_init();
    log::info!("start 1");
    let (send, recv) = channel::<Bytes>();
    log::info!("start 2");
    let sender_outbound = send.clone();
    log::info!("start 3");

    std::thread::spawn(|| {
        match create_pipeline(send).and_then(main_loop) {
            Ok(r) => r,
            Err(e) => eprintln!("Error! {}", e),
        };
    });
    log::info!("start 4");

    (sender_outbound, recv)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_records_rtp_via_udp() {
        let _ = env_logger::try_init();
        log::info!("it_records_rtp_via_udp 1");
        let (tx, rx) = start();
        log::info!("it_records_rtp_via_udp 2");
        let mut count = 0;
        let max = 10;

        while let Ok(_bytes) = rx.recv() {
            log::info!("it_records_rtp_via_udp loop");
            count += 1;

            if count >= max {
                break;
            }
        }
        log::info!("it_records_rtp_via_udp 3");

        assert_eq!(count, max);
    }
}
