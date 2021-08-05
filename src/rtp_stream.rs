use crate::error::Result;
use crate::main_loop::main_loop_simple;
use crate::{create_pipeline, element};
use byte_slice_cast::*;
use bytes::Bytes;
use gstreamer::element_error;
use gstreamer_app::{AppSink, AppSinkCallbacks};
use std::sync::mpsc::{channel, Receiver, Sender};

fn pipeline(sender: Sender<Bytes>) -> Result<gstreamer::Pipeline> {
    let launch = format!(
        "videotestsrc ! video/x-raw,format=I420,framerate=30/1,width=1280,height=720 ! x264enc tune=zerolatency ! rtph264pay ! appsink name=sink"
    );

    let pipeline = create_pipeline(&launch)?;
    let appsink = element::<AppSink>(&pipeline, "sink")?;

    // Getting data out of the appsink is done by setting callbacks on it.
    // The appsink will then call those handlers, as soon as data is available.
    appsink.set_callbacks(
        AppSinkCallbacks::builder()
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
                // memory region we mapped as an array of u8 packets.
                let samples = map.as_slice_of::<u8>().map_err(|_| {
                    element_error!(
                        appsink,
                        gstreamer::ResourceError::Failed,
                        ("Failed to interprete buffer as S16 PCM")
                    );

                    gstreamer::FlowError::Error
                })?;

                // not an error, just the receiver is no longer around
                if let Err(_) = sender.send(Bytes::from(samples.to_owned())) {
                    log::info!("Receiver not able to receive bytes from the rtp stream");
                };

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );

    Ok(pipeline)
}

pub fn start() -> (Sender<Bytes>, Receiver<Bytes>) {
    let _ = env_logger::try_init();
    let (send, recv) = channel::<Bytes>();
    let sender_outbound = send.clone();

    std::thread::spawn(|| {
        match pipeline(send).and_then(main_loop_simple) {
            Ok(r) => r,
            Err(e) => log::error!("Error! {}", e),
        };
    });

    (sender_outbound, recv)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_streams_rtp() {
        let (_tx, rx) = start();
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
