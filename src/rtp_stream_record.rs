use crate::main_loop::{main_loop, Command};
use anyhow::Error;
use bytes::Bytes;
use gstreamer::prelude::*;
use std::sync::mpsc::{Receiver, Sender};

fn create_pipeline(
    filename: &str,
    receiver: Receiver<Bytes>,
) -> Result<gstreamer::Pipeline, Error> {
    gstreamer::init()?;

    let pipeline = gstreamer::parse_launch(&format!(
        "appsrc name=src  \
        ! application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, payload=(int)96
        ! queue \
            ! rtph264depay name=pay0 \
            ! h264parse config-interval=-1 \
            ! mp4mux \
            ! filesink location={}",
        filename
    ))?
    .downcast::<gstreamer::Pipeline>()
    .expect("Expected a gstreamer::Pipeline");

    let appsrc = pipeline
        .by_name("src")
        .expect("src element not found")
        .downcast::<gstreamer_app::AppSrc>()
        .expect("src element is expected to be an appsrc");

    // let caps = gstreamer::Caps::new_simple(
    //     "video/x-raw",
    //     &[
    //         ("format", &"I420"),
    //         ("width", &1280),
    //         ("height", &720),
    //         ("framerate", &gstreamer::Fraction::new(30, 1)),
    //         ("pixel-aspect-ratio", &gstreamer::Fraction::new(1, 1)),
    //         ("interlace-mode", &"progressive"),
    //         ("chroma-site", &"mpeg2"),
    //         ("colorimetry", &"bt709"),
    //     ],
    // );

    // let caps = gstreamer::Caps::new_simple(
    //     "video/x-h264",
    //     &[
    //         ("format", &"avc"),
    //         ("width", &1280),
    //         ("height", &720),
    //         ("framerate", &gstreamer::Fraction::new(30, 1)),
    //         ("pixel-aspect-ratio", &gstreamer::Fraction::new(1, 1)),
    //         ("interlace-mode", &"progressive"),
    //         ("chroma-site", &"mpeg2"),
    //         ("colorimetry", &"bt709"),
    //     ],
    // );

    let caps = gstreamer::Caps::new_simple(
        "application/x-rtp",
        &[
            ("format", &"H264"),
            ("media", &"video"),
            ("clock-rate", &90000),
            ("payload", &96),
            ("chroma-site", &"mpeg2"),
            ("colorimetry", &"bt709"),
        ],
    );

    appsrc.set_caps(Some(&caps));
    appsrc.set_format(gstreamer::Format::Time);

    // write to the buffer in a separate thread to the appsrc in demand
    // it is not required to use the need_data callback
    std::thread::spawn(move || {
        let mut count = 1;
        while let Ok(bytes) = receiver.recv() {
            log::info!("received bytes: {:?}", bytes);

            let mut buffer = gstreamer::Buffer::from_slice(bytes);
            // For each frame we produce, we set the timestamp when it should be displayed
            // (pts = presentation time stamp)
            // The autovideosink will use this information to display the frame at the right time.
            // TODO: figure out a way to make the below more accurate
            let pts = count * 2500 * gstreamer::ClockTime::USECOND;
            buffer.get_mut().unwrap().set_pts(pts);

            if let Err(error) = appsrc.push_buffer(buffer) {
                log::error!("{:?}", error);
                let _ = appsrc.end_of_stream();
                break;
            }

            count += 1;
        }
    });

    Ok(pipeline)
}

pub fn record(
    filename: &str,
    receiver: Receiver<Bytes>,
    inbound_receiver: Receiver<Command>,
    outbound_sender: Sender<Command>,
) -> Result<(), Error> {
    let _ = env_logger::try_init();

    log::info!("Starting to record {}", filename);

    create_pipeline(filename, receiver)
        .and_then(|pipeline| main_loop(pipeline, inbound_receiver, outbound_sender))
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::rtp_stream::start;
    use std::path::Path;
    use std::sync::mpsc::channel;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn it_records_rtp_via_stream() {
        env_logger::try_init().unwrap();

        let filename = "test/output/it_records_rtp_via_stream.mp4";
        let (inbound_sender, inbound_receiver) = channel::<Command>();
        let (outbound_sender, outbound_receiver) = channel::<Command>();

        // start the rtp stream
        let (_tx, rx) = start();

        // record the video in a separate thread
        std::thread::spawn(move || {
            record(filename, rx, inbound_receiver, outbound_sender).unwrap();
        });

        // record for 4 seconds
        sleep(Duration::from_millis(4000));

        // stop recording
        inbound_sender.send(Command::Stop).unwrap();

        // listen for commands
        while let Ok(command) = outbound_receiver.recv() {
            match command {
                Command::Stopped => {
                    log::info!("received Command::Stopped");
                    // not a great assertion, but means we got to the end and the file exists
                    assert!(Path::new(filename).exists());
                    break;
                }
                _ => {}
            }
        }
    }
}
