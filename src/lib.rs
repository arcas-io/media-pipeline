pub mod error;
pub mod main_loop;
pub mod rtp_stream;
pub mod rtp_stream_record;
pub mod rtp_udp_client_record;
pub mod rtp_udp_server;

use std::sync::mpsc::{Receiver, Sender};

use crate::{error::{MediaPipelineError, Result}, main_loop::main_loop_simple};
use byte_slice_cast::{AsByteSlice, AsSliceOf};
use bytes::BytesMut;
use gstreamer::{Element, Pipeline, element_error, parse_launch, prelude::*};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use log::debug;

// Initialize gstreamer and create a pipeline
pub(crate) fn create_pipeline(launch: &str) -> Result<Pipeline> {
    gstreamer::init()?;

    let pipeline = parse_launch(&launch)
        .map_err(|e| MediaPipelineError::ParseLaunchError(e.to_string()))?
        .downcast::<Pipeline>()
        .map_err(|_| MediaPipelineError::CreatePipelineError)?;

    Ok(pipeline)
}

// Retrieve an element from the pipeline and downcast it
fn element<T: IsA<Element>>(pipeline: &Pipeline, name: &'static str) -> Result<T> {
    pipeline
        .by_name(name)
        .ok_or(MediaPipelineError::CreateElementError(name))?
        .downcast::<T>()
        .map_err(|_| MediaPipelineError::DowncastElementError(name))
}

fn appsink_pipeline(launch: &str, sender: Sender<BytesMut>) -> Result<gstreamer::Pipeline> {
    let launch = format!("{} ! appsink name=sink", launch);

    debug!("creating pipeline: {}", launch);

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
                if let Err(_) = sender.send(BytesMut::from(samples.to_owned().as_byte_slice())) {
                    log::info!("Receiver not able to receive bytes from the rtp stream");
                };

                Ok(gstreamer::FlowSuccess::Ok)
            })
            .build(),
    );
    debug!("set pipeline callbacks");

    Ok(pipeline)
}

pub fn create_and_start_appsink_pipeline(launch: &str) -> Result<Receiver<BytesMut>> {
    let (tx, rx) = std::sync::mpsc::channel::<BytesMut>();
    let pipline = appsink_pipeline(launch, tx);
    std::thread::spawn(move || {
        match pipline.and_then(main_loop_simple) {
            Ok(_) => {},
            Err(err) => log::error!("pipeline error: {}", err),
        }
    });
    Ok(rx)
}