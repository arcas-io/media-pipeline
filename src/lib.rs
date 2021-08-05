pub mod error;
pub mod main_loop;
pub mod rtp_stream;
pub mod rtp_stream_record;
pub mod rtp_udp_client_record;
pub mod rtp_udp_server;

use crate::error::{MediaPipelineError, Result};
use gstreamer::{parse_launch, prelude::*, Element, Pipeline};

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
