use crate::error::Result;
use crate::main_loop::{main_loop, Command};
use crate::{create_pipeline, element};
use bytes::BytesMut;
use glib::MainLoop;
use gstreamer::Pipeline;
use gstreamer_app::AppSrc;
use std::sync::mpsc::{Receiver, Sender};

fn pipeline(filename: &str, receiver: Receiver<BytesMut>) -> Result<Pipeline> {
    // TODO: handle different formats
    // TODO: research sdpdemux for handling flexible formats
    let launch = format!(
        "appsrc name=src  \
        ! application/x-rtp, media=(string)video, clock-rate=(int)90000, encoding-name=(string)H264, payload=(int)96
        ! queue \
            ! rtph264depay name=pay0 \
            ! h264parse config-interval=-1 \
            ! mp4mux \
            ! filesink location={}",
        filename
    );

    let pipeline = create_pipeline(&launch)?;
    let appsrc = element::<AppSrc>(&pipeline, "src")?;

    // I got these caps by running the stream creation on the command line and
    // used the last caps output.
    //
    // run this:
    //
    // gst-launch-1.0 -v videotestsrc ! video/x-raw,format=I420,framerate=30/1,width=1280,height=720 ! x264enc tune=zerolatency ! rtph264pay ! autovideosink
    //
    // then this is output:
    //
    // Got context from element 'autovideosink0': gst.gl.GLDisplay=context, gst.gl.GLDisplay=(GstGLDisplay)"\(GstGLDisplayCocoa\)\ gldisplaycocoa0";
    // /GstPipeline:pipeline0/GstVideoTestSrc:videotestsrc0.GstPad:src: caps = video/x-raw, format=(string)I420, width=(int)1280, height=(int)720, framerate=(fraction)30/1, multiview-mode=(string)mono, pixel-aspect-ratio=(fraction)1/1, interlace-mode=(string)progressive
    // /GstPipeline:pipeline0/GstCapsFilter:capsfilter0.GstPad:src: caps =
    // Redistribute latency...
    // /GstPipeline:pipeline0/GstX264Enc:x264enc0.GstPad:sink: caps = video/x-raw, format=(string)I420, width=(int)1280, height=(int)720, framerate=(fraction)30/1, multiview-mode=(string)mono, pixel-aspect-ratio=(fraction)1/1, interlace-mode=(string)progressive
    // /GstPipeline:pipeline0/GstCapsFilter:capsfilter0.GstPad:sink: caps = video/x-raw, format=(string)I420, width=(int)1280, height=(int)720, framerate=(fraction)30/1, multiview-mode=(string)mono, pixel-aspect-ratio=(fraction)1/1, interlace-mode=(string)progressive
    // /GstPipeline:pipeline0/GstX264Enc:x264enc0.GstPad:src: caps = video/x-h264, codec_data=(buffer)0142c01fffe1001b6742c01fd9005005bb016a02020280000003008000001e478c192401000468cb8cb2, stream-format=(string)avc, alignment=(string)au, level=(string)3.1, profile=(string)constrained-baseline, width=(int)1280, height=(int)720, pixel-aspect-ratio=(fraction)1/1, framerate=(fraction)30/1, interlace-mode=(string)progressive, colorimetry=(string)bt709, chroma-site=(string)mpeg2, multiview-mode=(string)mono, multiview-flags=(GstVideoMultiviewFlagsSet)0:ffffffff:/right-view-first/left-flipped/left-flopped/right-flipped/right-flopped/half-aspect/mixed-mono
    //
    // TODO: figure out a way to get caps from sdp

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
            log::trace!("received bytes: {:?}", bytes);

            let mut buffer = gstreamer::Buffer::from_slice(bytes);
            // For each frame we produce, we set the timestamp when it should be displayed
            // (pts = presentation time stamp)
            // The autovideosink will use this information to display the frame at the right time.
            // TODO: figure out a way to make the below more accurate (2500 makes a single use cas work)
            let pts = count * 2500 * gstreamer::ClockTime::USECOND;

            if let Some(mut_buffer) = buffer.get_mut() {
                mut_buffer.set_pts(pts);
            }

            // not an error, just the buffer is flushing
            if let Err(error) = appsrc.push_buffer(buffer) {
                log::info!("Could not push to buffer: {:?}", error);
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
    receiver: Receiver<BytesMut>,
    inbound_receiver: Receiver<Command>,
    outbound_sender: Sender<Command>,
) -> Result<MainLoop> {
    log::info!("Starting to record {}", filename);

    pipeline(filename, receiver)
        .and_then(|pipeline| main_loop(pipeline, inbound_receiver, outbound_sender))
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
        env_logger::try_init().ok();

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
        sleep(Duration::from_millis(2000));

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
