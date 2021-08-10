use glib::Error as GlibError;
use gstreamer::StateChangeError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MediaPipelineError>;

#[derive(Error, Debug)]
pub enum MediaPipelineError {
    #[error("Could not create element: {0}")]
    CreateElementError(&'static str),

    #[error("Failed to create a pipeline")]
    CreatePipelineError,

    #[error("Could not downcast element: {0}")]
    DowncastElementError(&'static str),

    #[error("Error in glib: {0}")]
    GlibError(String),

    #[error("Failed to initialize GStreamer: {0}")]
    InitError(String),

    #[error("Failed to parse the launch: {0}")]
    ParseLaunchError(String),

    #[error("Failed to parse the launch: {0}")]
    StateChangeError(String),
}

impl From<GlibError> for MediaPipelineError {
    fn from(error: GlibError) -> Self {
        MediaPipelineError::GlibError(error.to_string())
    }
}

impl From<StateChangeError> for MediaPipelineError {
    fn from(error: StateChangeError) -> Self {
        MediaPipelineError::StateChangeError(error.to_string())
    }
}
