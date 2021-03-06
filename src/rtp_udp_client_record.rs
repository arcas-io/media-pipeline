use crate::create_pipeline;
use crate::error::Result;
use crate::main_loop::{main_loop, Command};
use crossbeam_channel::{Receiver, Sender};
use glib::MainLoop;
use gstreamer::Pipeline;

fn pipeline(port: &str, filename: &str) -> Result<Pipeline> {
    let launch = format!(
        "udpsrc port={}  \
            ! application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, payload=(int)96
            ! queue  \
                ! rtph264depay name=pay0 \
                ! h264parse config-interval=-1 \
                ! mp4mux \
            ! filesink location={}",
            port,
            filename
    );

    let pipeline = create_pipeline(&launch)?;

    Ok(pipeline)
}

pub fn record(
    port: &str,
    filename: &str,
    inbound_receiver: Receiver<Command>,
    outbound_sender: Sender<Command>,
) -> Result<MainLoop> {
    log::info!("Starting to record {} from port {}", filename, port);

    pipeline(port, filename)
        .and_then(|pipeline| main_loop(pipeline, inbound_receiver, outbound_sender))
}

#[cfg(test)]
mod tests {

    use crossbeam_channel::unbounded;

    use super::*;
    use crate::rtp_udp_server::start;
    use std::path::Path;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn it_records_rtp_via_udp() {
        env_logger::try_init().ok();

        let filename = "test/output/it_records_rtp_via_udp.mp4";
        let (inbound_sender, inbound_receiver) = unbounded::<Command>();
        let (outbound_sender, outbound_receiver) = unbounded::<Command>();

        // start a udp server
        std::thread::spawn(move || {
            let (_, _) = start();
        });

        // record the video in a separate thread
        std::thread::spawn(move || {
            record("5000", filename, inbound_receiver, outbound_sender).unwrap();
        });

        // record for 2 seconds
        sleep(Duration::from_millis(2000));

        // stop recording
        inbound_sender.send(Command::Stop).unwrap();

        // listen for commands
        while let Ok(command) = outbound_receiver.recv() {
            if let Command::Stopped = command {
                log::info!("received Command::Stopped");
                assert!(Path::new(filename).exists());
                break;
            }
        }
    }
}
