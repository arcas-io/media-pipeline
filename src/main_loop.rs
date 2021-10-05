use crate::error::Result;
use gstreamer::prelude::*;
use gstreamer::Pipeline;
use std::sync::mpsc::{Receiver, Sender};

// Commands sent from and to the main loop
// TODO: add strum for auto string conversions
pub enum Command {
    // Stop ecording
    Stop,

    // Recording has stopped
    Stopped,
}

// Creates a new main_loop that is able to send and receive Commands
pub(crate) fn main_loop(
    pipeline: Pipeline,
    inbound_receiver: Receiver<Command>,
    outbound_sender: Sender<Command>,
) -> Result<glib::MainLoop> {
    let main_loop = glib::MainLoop::new(None, false);
    pipeline.set_state(gstreamer::State::Playing)?;

    // failing to create a bus is a catastrophic failure, panic
    let bus = pipeline
        .bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    let pipeline_weak = pipeline.downgrade();
    let log_command = |command: &str| log::info!("received {} in main loop", command);

    // listen for commands
    std::thread::spawn(move || {
        while let Ok(command) = inbound_receiver.recv() {
            match command {
                Command::Stop => {
                    log_command("Command::Stop");

                    if let Some(pipeline) = pipeline_weak.upgrade() {
                        log::info!("sending EOS");

                        pipeline.send_event(gstreamer::event::Eos::new());

                        if let Err(error) = outbound_sender.send(Command::Stopped) {
                            log::error!(
                                "Error sending Command:Stopped from the main loop: {:?}",
                                error
                            )
                        }
                    } else {
                        log::error!("Could not upgrade pipeline_weak in main loop");
                    }
                }
                _ => log::error!("Unhandled command"),
            }
        }
    });

    let main_loop_clone = main_loop.clone();

    // failing to add_watch to bus is a catastrophic failure, panic
    bus.add_watch(move |_, msg| {
        use gstreamer::MessageView;
        let main_loop = &main_loop_clone;

        let _view = match msg.view() {
            MessageView::Eos(..) => {
                log::info!("received EOS");
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

// Creates a new, basic main_loop that is used for trivial implementations
pub(crate) fn main_loop_simple(pipeline: gstreamer::Pipeline) -> Result<()> {
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
                log::error!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            _ => (),
        }
    }

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}

#[cfg(test)]
mod tests {}
