pub(crate) mod runloop;

use gstreamer::prelude::*;

fn build_pipeline() {
    // Initialize GStreamer
    if let Err(err) = gstreamer::init() {
        eprintln!("Failed to initialize Gst: {}", err);
        return;
    }

    let audio_source =
        gstreamer::ElementFactory::make("audiotestsrc", Some("audio_source")).unwrap();
    let tee = gstreamer::ElementFactory::make("tee", Some("tee")).unwrap();
    let audio_queue = gstreamer::ElementFactory::make("queue", Some("audio_queue")).unwrap();
    let audio_convert =
        gstreamer::ElementFactory::make("audioconvert", Some("audio_convert")).unwrap();
    let audio_resample =
        gstreamer::ElementFactory::make("audioresample", Some("audio_resample")).unwrap();
    let audio_sink = gstreamer::ElementFactory::make("autoaudiosink", Some("audio_sink")).unwrap();
    let video_queue = gstreamer::ElementFactory::make("queue", Some("video_queue")).unwrap();
    let visual = gstreamer::ElementFactory::make("wavescope", Some("visual")).unwrap();
    let video_convert =
        gstreamer::ElementFactory::make("videoconvert", Some("video_convert")).unwrap();
    let video_sink = gstreamer::ElementFactory::make("autovideosink", Some("video_sink")).unwrap();

    let pipeline = gstreamer::Pipeline::new(Some("test-pipeline"));

    audio_source.set_property("freq", &215.0).unwrap();
    visual.set_property_from_str("shader", "none");
    visual.set_property_from_str("style", "lines");

    pipeline
        .add_many(&[
            &audio_source,
            &tee,
            &audio_queue,
            &audio_convert,
            &audio_resample,
            &audio_sink,
            &video_queue,
            &visual,
            &video_convert,
            &video_sink,
        ])
        .unwrap();

    gstreamer::Element::link_many(&[&audio_source, &tee]).unwrap();
    gstreamer::Element::link_many(&[&audio_queue, &audio_convert, &audio_resample, &audio_sink])
        .unwrap();
    gstreamer::Element::link_many(&[&video_queue, &visual, &video_convert, &video_sink]).unwrap();

    let tee_audio_pad = tee.request_pad_simple("src_%u").unwrap();
    println!(
        "Obtained request pad {} for audio branch",
        tee_audio_pad.name()
    );
    let queue_audio_pad = audio_queue.static_pad("sink").unwrap();
    tee_audio_pad.link(&queue_audio_pad).unwrap();

    let tee_video_pad = tee.request_pad_simple("src_%u").unwrap();
    println!(
        "Obtained request pad {} for video branch",
        tee_video_pad.name()
    );
    let queue_video_pad = video_queue.static_pad("sink").unwrap();
    tee_video_pad.link(&queue_video_pad).unwrap();

    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    let bus = pipeline.bus().unwrap();

    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.src().map(|s| s.path_string()),
                    err.error()
                );
                eprintln!("Debugging information: {:?}", err.debug());
                break;
            }
            MessageView::Eos(..) => break,
            _ => (),
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}

fn main() {
    let _ = env_logger::try_init();
    crate::runloop::run(build_pipeline);
}
