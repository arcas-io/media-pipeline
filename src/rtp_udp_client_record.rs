use anyhow::Error;
use bytes::Bytes;
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

pub enum Command {
    Stop,
    Stopped,
}

fn create_pipeline() -> Result<gstreamer::Pipeline, Error> {
    gstreamer::init()?;
    log::info!("create_pipeline 1");

    let pipeline = gstreamer::parse_launch(&format!(
        "udpsrc port=5000  \
            ! application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, payload=(int)96
            ! queue  \
                ! rtph264depay name=pay0 \
                ! h264parse config-interval=-1 \
                ! mp4mux \
            ! filesink location=test.mp4"
    ))?
    .downcast::<gstreamer::Pipeline>()
    .expect("Expected a gst::Pipeline");
    log::info!("create_pipeline 2");

    Ok(pipeline)
}

fn main_loop(
    pipeline: gstreamer::Pipeline,
    sender: Sender<Command>,
    receiver: Receiver<Command>,
) -> Result<glib::MainLoop, Error> {
    log::info!("main loop");
    let main_loop = glib::MainLoop::new(None, false);
    pipeline.set_state(gstreamer::State::Playing)?;

    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    let pipeline_weak = pipeline.downgrade();

    // listen for commands
    std::thread::spawn(move || {
        while let Ok(command) = receiver.recv() {
            match command {
                Command::Stop => {
                    let pipeline = pipeline_weak.upgrade().unwrap();

                    println!("sending eos");

                    pipeline.send_event(gstreamer::event::Eos::new());

                    glib::Continue(false);
                    sender.send(Command::Stopped).unwrap();
                }
                _ => {}
            }
        }
    });

    // glib::timeout_add_seconds(1, move || {
    //     let pipeline = match pipeline_weak.upgrade() {
    //         Some(pipeline) => pipeline,
    //         None => return glib::Continue(false),
    //     };

    //     println!("sending eos");

    //     pipeline.send_event(gstreamer::event::Eos::new());

    //     glib::Continue(false)
    // });

    let main_loop_clone = main_loop.clone();

    bus.add_watch(move |_, msg| {
        use gstreamer::MessageView;
        let main_loop = &main_loop_clone;

        let _view = match msg.view() {
            MessageView::Eos(..) => {
                println!("received eos");
                // An EndOfStream event was sent to the pipeline, so we tell our main loop
                // to stop execution here.
                main_loop.quit()
            }
            MessageView::Error(err) => {
                println!(
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

pub fn stop(pipeline: Pipeline) -> Result<(), Error> {
    log::info!("stop");

    let pipeline_weak = pipeline.downgrade();
    let pipeline = pipeline_weak.upgrade().unwrap();

    println!("sending eos");

    pipeline.send_event(gstreamer::event::Eos::new());

    glib::Continue(false);

    Ok(())
}

pub fn start(sender: Sender<Command>, receiver: Receiver<Command>) -> Result<(), Error> {
    let _ = env_logger::try_init();
    log::info!("start 1");
    log::info!("start 2");

    create_pipeline()
        .and_then(|pipeline| main_loop(pipeline, sender, receiver))
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn it_records_rtp_via_udp() {
        let _ = env_logger::try_init();
        log::info!("it_records_rtp_via_udp 1");

        let (tx, rx) = std::sync::mpsc::channel::<Command>();
        let sender = tx.clone();

        std::thread::spawn(|| {
            start(sender, rx).unwrap();
        });

        // record for 2 seconds
        sleep(Duration::from_millis(2000));

        // stop recording
        tx.send(Command::Stop).unwrap();

        // finish pause for stop to finish
        // TODO: receive message from the main loop rather than wait
        sleep(Duration::from_millis(1000));

        // assert_eq!(count, max);
    }
}
