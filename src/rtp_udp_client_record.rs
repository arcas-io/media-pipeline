use anyhow::Error;
use derive_more::{Display, Error};
use gstreamer::prelude::*;
use gstreamer::Pipeline;
use std::sync::mpsc::{Receiver, Sender};

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

type ChannelPair = (Sender<Command>, Receiver<Command>);

// Commands sent from and to the main loop
pub enum Command {
    // Stop ecording
    Stop,

    // Recording has stopped
    Stopped,
}

fn create_pipeline(filename: &str) -> Result<Pipeline, Error> {
    gstreamer::init()?;

    let pipeline = gstreamer::parse_launch(&format!(
        "udpsrc port=5000  \
            ! application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, payload=(int)96
            ! queue  \
                ! rtph264depay name=pay0 \
                ! h264parse config-interval=-1 \
                ! mp4mux \
            ! filesink location={}",
            filename
    ))?
    .downcast::<Pipeline>()
    .expect("Expected a gst::Pipeline");

    Ok(pipeline)
}

fn main_loop(
    pipeline: Pipeline,
    inbound_receiver: Receiver<Command>,
    outbound_sender: Sender<Command>,
) -> Result<glib::MainLoop, Error> {
    let main_loop = glib::MainLoop::new(None, false);
    pipeline.set_state(gstreamer::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    let pipeline_weak = pipeline.downgrade();

    // listen for commands
    std::thread::spawn(move || {
        while let Ok(command) = inbound_receiver.recv() {
            match command {
                Command::Stop => {
                    log::info!("received Command::Stop");
                    let pipeline = pipeline_weak.upgrade().unwrap();

                    log::info!("sending eos");

                    pipeline.send_event(gstreamer::event::Eos::new());

                    glib::Continue(false);
                    outbound_sender.send(Command::Stopped).unwrap();
                }
                _ => {}
            }
        }
    });

    let main_loop_clone = main_loop.clone();

    bus.add_watch(move |_, msg| {
        use gstreamer::MessageView;
        let main_loop = &main_loop_clone;

        let _view = match msg.view() {
            MessageView::Eos(..) => {
                log::info!("received eos");
                // An EndOfStream event was sent to the pipeline, so we tell our main loop
                // to stop execution here.
                main_loop.quit()
            }
            MessageView::Error(err) => {
                log::error!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                main_loop.quit();
            }
            _ => (),
        };

        glib::Continue(true)
    })
    .expect("Failed to add bus watch");

    main_loop.run();

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(main_loop)
}

pub fn record(
    filename: &str,
    inbound_receiver: Receiver<Command>,
    outbound_sender: Sender<Command>,
) -> Result<(), Error> {
    let _ = env_logger::try_init();

    create_pipeline(filename)
        .and_then(|pipeline| main_loop(pipeline, inbound_receiver, outbound_sender))
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::rtp_udp_server::start;
    use std::path::Path;
    use std::sync::mpsc::channel;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn it_records_rtp_via_udp() {
        let _ = env_logger::try_init();

        let filename = "test/output/it_records_rtp_via_udp.mp4";
        let (inbound_sender, inbound_receiver) = channel::<Command>();
        let (outbound_sender, outbound_receiver) = channel::<Command>();

        // start a udp server
        std::thread::spawn(move || {
            let (_, _) = start();
        });

        // record the video in a separate thread
        std::thread::spawn(move || {
            record(filename, inbound_receiver, outbound_sender).unwrap();
        });

        // record for 2 seconds
        sleep(Duration::from_millis(2000));

        // stop recording
        inbound_sender.send(Command::Stop).unwrap();

        // listen for commands
        while let Ok(command) = outbound_receiver.recv() {
            match command {
                Command::Stopped => {
                    log::info!("received Command::Stopped");
                    assert!(Path::new(filename).exists());
                    break;
                }
                _ => {}
            }
        }
    }
}
