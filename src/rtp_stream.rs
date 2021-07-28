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

    let pipeline = gstreamer::parse_launch(&format!(
        "videotestsrc ! x264enc tune=zerolatency ! rtph264pay ! appsink name=sink"
    ))?
    .downcast::<gstreamer::Pipeline>()
    .expect("Expected a gst::Pipeline");

    let appsink = pipeline
        .by_name("sink")
        .expect("Sink element not found")
        .downcast::<gstreamer_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");

    // Getting data out of the appsink is done by setting callbacks on it.
    // The appsink will then call those handlers, as soon as data is available.
    appsink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            // Add a handler to the "new-sample" signal.
            .new_sample(move |appsink| {
                // Pull the sample in question out of the appsink's buffer.
                let sample = appsink
                    .pull_sample()
                    .map_err(|_| gstreamer::FlowError::Eos)?;

                let buffer = sample.buffer().ok_or_else(|| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to get buffer from appsink")
                    );

                    gstreamer::FlowError::Error
                })?;

                // At this point, buffer is only a reference to an existing memory region somewhere.
                // When we want to access its content, we have to map it while requesting the required
                // mode of access (read, read/write).
                // This type of abstraction is necessary, because the buffer in question might not be
                // on the machine's main memory itself, but rather in the GPU's memory.
                // So mapping the buffer makes the underlying memory region accessible to us.
                // See: https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/allocation.html
                let map = buffer.map_readable().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to map buffer readable")
                    );

                    gstreamer::FlowError::Error
                })?;

                // We know what format the data in the memory region has, since we requested
                // it by setting the appsink's caps. So what we do here is interpret the
                // memory region we mapped as an array of signed 16 bit integers.
                let samples = map.as_slice_of::<u8>().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to interprete buffer as S16 PCM")
                    );

                    gstreamer::FlowError::Error
                })?;

                // TODO: remove unwrap
                sender.send(Bytes::from(samples.to_owned())).unwrap();

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );

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
    fn it_streams_rtp() {
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
