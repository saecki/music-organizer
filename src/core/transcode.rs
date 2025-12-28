use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{bail, Context};
use crossbeam_channel::Sender;
use ffmpeg::{codec, filter, format, frame, media};

use crate::thread::{worker_pool, Msg, WorkerState};
use crate::{AudioFormat, MusicIndex, Song};

/// 256 kbit/s
const TRANSCODE_BIT_RATE_TARGET: usize = 256_000;
const TRANSCODE_BIT_RATE_MAX: usize = 320_000;

pub fn transcode_songs(
    index: &MusicIndex,
    output_dir: &Path,
    f: &mut impl FnMut(TranscodeOp, anyhow::Result<()>),
) {
    let (item_sender, item_receiver) = crossbeam_channel::unbounded();

    let num_workers = std::thread::available_parallelism().unwrap().get().max(2) - 1;
    worker_pool(
        num_workers,
        index.songs.iter(),
        |_| TrancodeWorker { sender: item_sender.clone(), music_dir: index.music_dir, output_dir },
        || {
            while let Ok(Msg::Work((op, res))) = item_receiver.recv() {
                f(op, res);
            }
        },
    );
}

pub struct TranscodeOp<'a> {
    pub new_path: PathBuf,
    pub song: &'a Song,
    /// The format in which the song will be transcoded.
    pub format: Option<AudioFormat>,
}

impl AudioFormat {
    fn codec_id(&self) -> ffmpeg::codec::Id {
        match self {
            AudioFormat::Flac => ffmpeg::codec::Id::FLAC,
            AudioFormat::M4a => ffmpeg::codec::Id::AAC,
            AudioFormat::Mp3 => ffmpeg::codec::Id::MP3,
            AudioFormat::Opus => ffmpeg::codec::Id::OPUS,
        }
    }

    fn rate(&self) -> u32 {
        match self {
            AudioFormat::Flac => 44_100,
            AudioFormat::M4a => 44_100,
            AudioFormat::Mp3 => 44_100,
            AudioFormat::Opus => 48_000,
        }
    }
}

fn determine_transcode_format(song: &Song) -> Option<AudioFormat> {
    match song.format {
        AudioFormat::Flac => Some(AudioFormat::Opus),
        AudioFormat::M4a => None,
        AudioFormat::Mp3 => None,
        AudioFormat::Opus => None,
    }
}

struct TrancodeWorker<'a> {
    sender: Sender<Msg<(TranscodeOp<'a>, anyhow::Result<()>)>>,
    music_dir: &'a Path,
    output_dir: &'a Path,
}

impl<'a> WorkerState<&'a Song> for TrancodeWorker<'a> {
    fn work(worker: &mut crate::thread::Worker<Self, &'a Song>, song: &'a Song) {
        let format = determine_transcode_format(song);

        let sub_path = song
            .path
            .strip_prefix(worker.state.music_dir)
            .expect("All songs should be located inside the `music-dir`");
        let mut new_path = worker.state.output_dir.join(sub_path);
        if let Some(format) = format {
            new_path.set_extension(format.extension());
        }

        // TODO: Compute changes beforehand and display them.
        if new_path.exists() {
            return;
        }

        let op = TranscodeOp { new_path, song, format };
        let res = execute_transcode_op(&op);

        worker.state.sender.send(Msg::Work((op, res))).unwrap();
    }

    fn stop(&mut self) {
        self.sender.send(Msg::Stop).unwrap();
    }
}

fn execute_transcode_op(op: &TranscodeOp) -> anyhow::Result<()> {
    if let Some(parent) = op.new_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match op.format {
        Some(format) => {
            // FIXME: Write into buffer and write metadata in memory.
            transcode_song(op.song, &op.new_path, format)?;
            op.song.write_metadata_to(&op.new_path, format)?;
        }
        None => {
            std::fs::copy(&op.song.path, &op.new_path)?;
        }
    }
    Ok(())
}

fn transcode_song(song: &Song, new_path: &Path, format: AudioFormat) -> anyhow::Result<()> {
    static INIT_FFMPEG: LazyLock<()> = LazyLock::new(|| {
        ffmpeg::log::set_level(ffmpeg::log::Level::Warning);
        ffmpeg::init().unwrap();
    });
    LazyLock::force(&INIT_FFMPEG);

    let mut ictx = format::input(&song.path).context("error opening input file")?;
    let mut octx = format::output(&new_path).context("error opening output file")?;
    let mut transcoder =
        transcoder(&mut ictx, &mut octx, format).context("error building transcoder")?;

    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header().context("error writing header")?;

    for (stream, mut packet) in ictx.packets() {
        if stream.index() == transcoder.stream {
            packet.rescale_ts(stream.time_base(), transcoder.in_time_base);
            transcoder.send_packet_to_decoder(&packet)?;
            transcoder.receive_and_process_decoded_frames(&mut octx)?;
        }
    }

    transcoder.send_eof_to_decoder()?;
    transcoder.receive_and_process_decoded_frames(&mut octx)?;

    transcoder.flush_filter()?;
    transcoder.get_and_process_filtered_frames(&mut octx)?;

    transcoder.send_eof_to_encoder()?;
    transcoder.receive_and_process_encoded_packets(&mut octx)?;

    octx.write_trailer()?;

    Ok(())
}

fn filter(
    format: AudioFormat,
    decoder: &codec::decoder::Audio,
    encoder: &codec::encoder::Audio,
) -> Result<filter::Graph, ffmpeg::Error> {
    let mut filter = filter::Graph::new();

    let args = format!(
        "time_base={}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
        decoder.time_base(),
        decoder.rate(),
        decoder.format().name(),
        decoder.channel_layout().bits()
    );

    filter.add(&filter::find("abuffer").unwrap(), "in", &args)?;
    filter.add(&filter::find("abuffersink").unwrap(), "out", "")?;

    {
        let mut out = filter.get("out").unwrap();

        out.set_sample_format(encoder.format());
        out.set_channel_layout(encoder.channel_layout());
        out.set_sample_rate(format.rate());
    }

    filter.output("in", 0)?.input("out", 0)?.parse("anull")?;
    filter.validate()?;

    if let Some(codec) = encoder.codec() {
        if !codec
            .capabilities()
            .contains(ffmpeg::codec::capabilities::Capabilities::VARIABLE_FRAME_SIZE)
        {
            filter.get("out").unwrap().sink().set_frame_size(encoder.frame_size());
        }
    }

    Ok(filter)
}

struct FfmpegTranscoder {
    stream: usize,
    filter: filter::Graph,
    decoder: codec::decoder::Audio,
    encoder: codec::encoder::Audio,
    in_time_base: ffmpeg::Rational,
    out_time_base: ffmpeg::Rational,
}

fn transcoder(
    ictx: &mut format::context::Input,
    octx: &mut format::context::Output,
    format: AudioFormat,
) -> anyhow::Result<FfmpegTranscoder> {
    let num_audio_streams =
        ictx.streams().filter(|s| s.parameters().medium() == media::Type::Audio).count() as u32;
    if num_audio_streams != 1 {
        bail!("Wrong number of audio streams, expected excatly 1 but found {num_audio_streams}");
    }

    let input = ictx.streams().best(media::Type::Audio).expect("could not find best audio stream");
    let context = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context.decoder().audio()?;
    let codec =
        ffmpeg::encoder::find(format.codec_id()).expect("failed to find encoder").audio()?;
    let global = octx.format().flags().contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER);

    decoder.set_parameters(input.parameters())?;

    let mut output = octx.add_stream(codec)?;
    let mut context = ffmpeg::codec::context::Context::from_parameters(output.parameters())?;
    if format == AudioFormat::Opus {
        // `compression_level` for `libopus` is in the range of `0..=10`
        let avctx = unsafe { &mut *context.as_mut_ptr() };
        avctx.compression_level = 10;
    }
    let mut encoder = context.encoder().audio()?;

    let channel_layout = codec
        .channel_layouts()
        .map(|cls| cls.best(decoder.channel_layout().channels()))
        .unwrap_or(ffmpeg::channel_layout::ChannelLayout::STEREO);

    if global {
        encoder.set_flags(ffmpeg::codec::flag::Flags::GLOBAL_HEADER);
    }

    encoder.set_rate(format.rate() as i32);
    encoder.set_channel_layout(channel_layout);
    encoder.set_format(codec.formats().expect("unknown supported formats").next().unwrap());
    encoder.set_bit_rate(TRANSCODE_BIT_RATE_TARGET);
    encoder.set_max_bit_rate(TRANSCODE_BIT_RATE_MAX);

    encoder.set_time_base((1, format.rate() as i32));
    output.set_time_base((1, format.rate() as i32));

    let encoder = encoder.open_as(codec)?;
    output.set_parameters(&encoder);

    let filter = filter(format, &decoder, &encoder).context("error building filter")?;

    let in_time_base = decoder.time_base();
    let out_time_base = output.time_base();

    Ok(FfmpegTranscoder {
        stream: input.index(),
        filter,
        decoder,
        encoder,
        in_time_base,
        out_time_base,
    })
}

impl FfmpegTranscoder {
    fn send_frame_to_encoder(&mut self, frame: &ffmpeg::Frame) -> Result<(), ffmpeg::Error> {
        self.encoder.send_frame(frame)
    }

    fn send_eof_to_encoder(&mut self) -> Result<(), ffmpeg::Error> {
        self.encoder.send_eof()
    }

    fn receive_and_process_encoded_packets(
        &mut self,
        octx: &mut format::context::Output,
    ) -> Result<(), ffmpeg::Error> {
        let mut encoded = ffmpeg::Packet::empty();
        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            encoded.rescale_ts(self.in_time_base, self.out_time_base);
            encoded.write_interleaved(octx)?;
        }
        Ok(())
    }

    fn add_frame_to_filter(&mut self, frame: &ffmpeg::Frame) -> Result<(), ffmpeg::Error> {
        self.filter.get("in").unwrap().source().add(frame)
    }

    fn flush_filter(&mut self) -> Result<(), ffmpeg::Error> {
        self.filter.get("in").unwrap().source().flush()
    }

    fn get_and_process_filtered_frames(
        &mut self,
        octx: &mut format::context::Output,
    ) -> Result<(), ffmpeg::Error> {
        let mut filtered = frame::Audio::empty();
        while self.filter.get("out").unwrap().sink().frame(&mut filtered).is_ok() {
            self.send_frame_to_encoder(&filtered)?;
            self.receive_and_process_encoded_packets(octx)?;
        }
        Ok(())
    }

    fn send_packet_to_decoder(&mut self, packet: &ffmpeg::Packet) -> Result<(), ffmpeg::Error> {
        self.decoder.send_packet(packet)
    }

    fn send_eof_to_decoder(&mut self) -> Result<(), ffmpeg::Error> {
        self.decoder.send_eof()
    }

    fn receive_and_process_decoded_frames(
        &mut self,
        octx: &mut format::context::Output,
    ) -> Result<(), ffmpeg::Error> {
        let mut decoded = frame::Audio::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            let timestamp = decoded.timestamp();
            decoded.set_pts(timestamp);
            self.add_frame_to_filter(&decoded)?;
            self.get_and_process_filtered_frames(octx)?;
        }
        Ok(())
    }
}
