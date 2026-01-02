use std::ffi::CString;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use crossbeam_channel::Sender;
use opusmeta::LowercaseString;
use rubato::Resampler;
use symphonia::core::audio::{Channels as SymphoniaChannels, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::thread::{worker_pool, Msg, WorkerState};
use crate::{meta, AudioFormat, MusicIndex, Song};

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
    pub format: Option<TranscodeFormat>,
}

/// The target format for transcoding songs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TranscodeFormat {
    Opus,
}

impl TranscodeFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            TranscodeFormat::Opus => "opus",
        }
    }
}

impl From<TranscodeFormat> for AudioFormat {
    fn from(value: TranscodeFormat) -> Self {
        match value {
            TranscodeFormat::Opus => AudioFormat::Opus,
        }
    }
}

fn determine_transcode_format(song: &Song) -> Option<TranscodeFormat> {
    match song.format {
        AudioFormat::Flac => Some(TranscodeFormat::Opus),
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
            let data = transcode_song(op.song, format)?;
            std::fs::write(&op.new_path, &data)?;
        }
        None => {
            std::fs::copy(&op.song.path, &op.new_path)?;
        }
    }
    Ok(())
}

/// Trancodes a song by using:
/// 1. symphonia for demuxing/decoding
/// 2. rubato for resampling to 48kHz from most likely 44.1kHz (in flac, aac, mp3, etc.)
/// 3. libopusenc to encode an ogg/opus file
fn transcode_song(song: &Song, format: TranscodeFormat) -> anyhow::Result<Vec<u8>> {
    let mut all_samples = Vec::new();
    let (rate, channels) = decode(&song.path, &mut all_samples)?;
    match format {
        TranscodeFormat::Opus => transcode_to_opus(song, rate, channels, &all_samples),
    }
}

#[derive(Clone, Copy, Debug)]
enum Channels {
    Mono = 1,
    Stereo = 2,
}

impl Channels {
    fn count(self) -> usize {
        self as usize
    }
}

fn decode(path: &Path, all_samples: &mut Vec<f32>) -> anyhow::Result<(u32, Channels)> {
    let codecs = symphonia::default::get_codecs();
    let probe = symphonia::default::get_probe();

    let raw_data = std::fs::read(path)?;
    let source = Cursor::new(raw_data);
    let stream = MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
    let probe_result = probe
        .format(&Hint::new(), stream, &FormatOptions::default(), &MetadataOptions::default())
        .context("unsupported format")?;
    let mut format_reader = probe_result.format;

    // TODO: Maybe filter out other tracks?
    let [track] = format_reader.tracks() else { bail!("expected exactly one audio track") };
    let track_id = track.id;
    let mut decoder = codecs
        .make(&track.codec_params, &DecoderOptions { verify: true })
        .context("unsupported codec")?;

    let audio_buf = loop {
        let packet = format_reader.next_packet()?;
        if packet.track_id() != track_id {
            continue;
        }

        break decoder.decode(&packet)?;
    };

    let spec = *audio_buf.spec();
    const STEREO_CHANNELS: SymphoniaChannels =
        SymphoniaChannels::FRONT_LEFT.union(SymphoniaChannels::FRONT_RIGHT);
    let channels = match spec.channels {
        c if c.count() == 1 => Channels::Mono,
        STEREO_CHANNELS => Channels::Stereo,
        layout => bail!("expected mono or stereo channel layout, found {layout:?}"),
    };

    let duration = audio_buf.capacity() as u64;
    let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);

    sample_buf.copy_interleaved_ref(audio_buf);
    all_samples.extend_from_slice(sample_buf.samples());

    while let Ok(packet) = format_reader.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        let audio_buf = decoder.decode(&packet)?;
        sample_buf.copy_interleaved_ref(audio_buf);
        all_samples.extend_from_slice(sample_buf.samples());
    }

    Ok((spec.rate, channels))
}

fn transcode_to_opus(
    song: &Song,
    sample_rate: u32,
    channels: Channels,
    in_samples: &[f32],
) -> anyhow::Result<Vec<u8>> {
    // Opus only supports sample rates of 8, 12, 16, 24 and 48kHz
    const OPUS_SAMPLE_RATE: usize = 48_000;

    let resampled = resample::<OPUS_SAMPLE_RATE>(sample_rate, channels, in_samples)
        .context("error resampling")?;

    // Build vorbis comments.
    let comments = {
        // Add audio metadata.
        let mut comments = opusenc::Comments::create();
        fn add_vorbis<V: ToString>(
            comments: &mut opusenc::Comments,
            key: &LowercaseString,
            value: Option<V>,
        ) -> anyhow::Result<()> {
            if let Some(value) = value {
                let key = CString::new(key.to_string().into_bytes()).unwrap();
                comments.add(key, value.to_string())?;
            }
            Ok(())
        }
        add_vorbis(&mut comments, &meta::opus::TRACKNUMBER, song.track_number)?;
        add_vorbis(&mut comments, &meta::opus::TOTALTRACKS, song.total_tracks)?;
        add_vorbis(&mut comments, &meta::opus::DISCNUMBER, song.disc_number)?;
        add_vorbis(&mut comments, &meta::opus::TOTALDISCS, song.total_discs)?;
        for artist in song.artists.iter() {
            add_vorbis(&mut comments, &meta::opus::ARTIST, Some(artist))?;
        }
        for artist in song.album_artists.iter() {
            add_vorbis(&mut comments, &meta::opus::ALBUMARTIST, Some(artist))?;
        }
        add_vorbis(&mut comments, &meta::opus::ALBUM, Some(&song.album))?;
        add_vorbis(&mut comments, &meta::opus::TITLE, Some(&song.title))?;
        for genre in song.genres.iter() {
            add_vorbis(&mut comments, &meta::opus::GENRE, Some(genre))?;
        }
        if song.has_artwork {
            let image = meta::read_image_from(&song.path, song.format)?;
            comments.add_picture_from_memory(
                &image,
                opusenc::PictureType::FrontCover,
                Option::<Vec<u8>>::None,
            )?;
        }
        comments
    };

    const OPUS_BITRATE: i32 = 256_000;

    // Setup ogg/opus encoder.
    let mut encoder = opusenc::Encoder::create_pull(
        comments,
        OPUS_SAMPLE_RATE as i32,
        channels.count(),
        opusenc::MappingFamily::MonoStereo,
    )?;
    encoder.set_bitrate(opusenc::Bitrate::Bits(OPUS_BITRATE))?;

    // Encode into ogg/opus container.
    let mut output = Vec::new();
    {
        encoder.write_float(&resampled)?;
        while let Some(page) = encoder.get_page(false) {
            output.extend_from_slice(page);
        }

        encoder.drain()?;
        while let Some(page) = encoder.get_page(true) {
            output.extend_from_slice(page);
        }
    }

    Ok(output)
}

fn resample<const OUT_RATE: usize>(
    sample_rate: u32,
    channels: Channels,
    in_samples: &[f32],
) -> anyhow::Result<Vec<f32>> {
    let mut resampler = rubato::Fft::new(
        sample_rate as usize,
        OUT_RATE,
        1024,
        1,
        channels.count(),
        rubato::FixedSync::Input,
    )?;

    let num_in_frames = in_samples.len() / channels.count();
    let mut sample_buf = {
        let len = resampler.process_all_needed_output_len(num_in_frames) * channels.count();
        vec![0.0; (1.2 * len as f64) as usize]
    };

    let num_out_frames = sample_buf.len() / channels.count();
    {
        let in_adapter = audioadapter_buffers::direct::InterleavedSlice::new(
            in_samples,
            channels.count(),
            num_in_frames,
        )?;

        let mut out_adapter = audioadapter_buffers::direct::InterleavedSlice::new_mut(
            &mut sample_buf,
            channels.count(),
            num_out_frames,
        )?;

        let (_in_processed, out_written) = resampler.process_all_into_buffer(
            &in_adapter,
            &mut out_adapter,
            num_in_frames,
            None,
        )?;

        sample_buf.truncate(out_written * channels.count());
    }

    Ok(sample_buf)
}
