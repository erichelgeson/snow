pub mod flux;
pub mod loaders;
mod macformat;

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_big_array::{Array, BigArray};
use strum::EnumIter;

use flux::FluxTicks;
pub use loaders::ImageType;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

/// Key/value collection of floppy metadata.
/// Loaders should convert keys to lowercase.
pub type FloppyMetadata = HashMap<String, String>;

/// Types of emulated floppies - 3.5" only
#[derive(Copy, Clone, EnumIter, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum FloppyType {
    /// Macintosh GCR CLV 3.5", single sided
    Mac400K,
    /// Macintosh GCR CLV 3.5", double sided
    Mac800K,
    /// MFM CAV 3.5", double sided, high density
    Mfm144M,
}

/// Type of the original track when loaded from the image
#[derive(Copy, Clone, EnumIter, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum OriginalTrackType {
    /// Unknown
    #[default]
    Unknown,
    /// Logical sector data
    Sector,
    /// Physical bitstream data
    Bitstream,
    /// Flux transitions, solved
    Flux,
    /// Flux transitions, raw
    RawFlux,
}

/// Current track type
#[derive(Debug, Copy, Clone, EnumIter, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum TrackType {
    /// Physical bitstream data
    #[default]
    Bitstream,
    /// Flux transitions
    Flux,
}

/// Length of a track in different units
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TrackLength {
    Bits(usize),
    Transitions(usize),
}

impl FloppyType {
    /// Gets the (approximate) track length in bits
    pub fn get_approx_track_length(self, track: usize) -> usize {
        match self {
            Self::Mac400K | Self::Mac800K => match track {
                0..=15 => 74640,
                16..=31 => 68240,
                32..=47 => 62200,
                48..=63 => 55980,
                64..=79 => 49760,
                _ => unreachable!(),
            },
            Self::Mfm144M => 192992,
        }
    }

    /// Gets the sector size for this format
    pub fn get_sector_size(self) -> usize {
        512
    }

    /// Gets the amount of sectors for this format
    pub fn get_sector_count(self) -> usize {
        self.get_logical_size() / self.get_sector_size()
    }

    /// Gets the logical size of the total image
    pub fn get_logical_size(self) -> usize {
        match self {
            Self::Mac400K => 400 * 1024,
            Self::Mac800K => 800 * 1024,
            Self::Mfm144M => 1440 * 1024,
        }
    }
}

impl std::fmt::Display for FloppyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Mac400K => "Macintosh GCR 400KB",
                Self::Mac800K => "Macintosh GCR 800KB",
                Self::Mfm144M => "MFM 1.44MB",
            }
        )
    }
}

const FLOPPY_MAX_SIDES: usize = 2;
const FLOPPY_MAX_TRACKS: usize = 80;

/// A read/writable floppy interface that can be used with Snow
/// This exposes the physical disk bits, as they are outputted by the IWM
pub trait Floppy {
    /// Gets the type of the emulated floppy
    fn get_type(&self) -> FloppyType;

    /// Gets the amount of tracks per side
    fn get_track_count(&self) -> usize;

    /// Gets a specific bit on a track and side
    fn get_track_bit(&self, side: usize, track: usize, position: usize) -> bool;

    /// Sets a specific bit on a track and side
    fn set_track_bit(&mut self, side: usize, track: usize, position: usize, value: bool);

    /// Gets the length of a specific track
    fn get_track_length(&self, side: usize, track: usize) -> TrackLength;

    /// Gets the amount of sides on the floppy
    fn get_side_count(&self) -> usize;

    /// A generic, user readable identification of the image
    /// For example: the title, label or filename
    fn get_title(&self) -> &str;

    /// Gets the metadata of the image as key/value.
    /// May be empty if the initial format did not support it or there is no metadata.
    fn get_metadata(&self) -> FloppyMetadata;

    /// Gets the write protect status of the medium.
    fn get_write_protect(&self) -> bool;
}

/// An in-memory loaded floppy image
#[derive(Clone, Serialize, Deserialize)]
pub struct FloppyImage {
    floppy_type: FloppyType,

    /// Bitstream track data
    /// Only tracks with bitstream accuracy are filled in here
    #[serde(with = "BigArray")]
    pub(crate) trackdata: [Array<Vec<u8>, FLOPPY_MAX_TRACKS>; FLOPPY_MAX_SIDES],

    /// Bit length of bitstream tracks
    #[serde(with = "BigArray")]
    bitlen: [Array<usize, FLOPPY_MAX_TRACKS>; FLOPPY_MAX_SIDES],

    /// Flux track data, stored in flux transition time (in ticks)
    /// Only tracks with flux accuracy are filled in here
    #[serde(with = "BigArray")]
    pub(crate) flux_trackdata: [Array<Vec<FluxTicks>, FLOPPY_MAX_TRACKS>; FLOPPY_MAX_SIDES],

    /// Original track types at load time
    #[serde(with = "BigArray")]
    pub(crate) origtracktype: [Array<OriginalTrackType, FLOPPY_MAX_TRACKS>; FLOPPY_MAX_SIDES],

    /// Some way to represent what is on this floppy (e.g. the label)
    title: String,

    /// Key/value store of metadata
    metadata: FloppyMetadata,

    /// Floppy has been written to and not saved since
    dirty: bool,

    /// Force floppy read-only
    force_wp: bool,

    /// Original image format this was loaded from (for save auto-detection)
    #[serde(default)]
    source_format: Option<ImageType>,
}

impl FloppyImage {
    /// Creates a new, empty image for the specified type
    /// Tracks are sized to their approximate size
    ///
    /// Image is filled with random noise
    pub fn new(floppy_type: FloppyType, title: &str) -> Self {
        let mut img = Self::new_internal(floppy_type, title);
        for side in 0..FLOPPY_MAX_SIDES {
            for track in 0..FLOPPY_MAX_TRACKS {
                img.set_actual_track_length(
                    side,
                    track,
                    floppy_type.get_approx_track_length(track),
                );
            }
        }
        img
    }

    /// Legacy function, kept for possible future differentiation to create
    /// images faster.
    pub fn new_empty(floppy_type: FloppyType, title: &str) -> Self {
        Self::new(floppy_type, title)
    }

    /// Creates a new, empty image for the specified type
    /// Tracks are sized to empty so they can be filled
    fn new_internal(floppy_type: FloppyType, title: &str) -> Self {
        Self {
            floppy_type,
            trackdata: core::array::from_fn(|_| Default::default()),
            flux_trackdata: core::array::from_fn(|_| Default::default()),
            bitlen: [Default::default(); FLOPPY_MAX_SIDES],
            title: title.to_owned(),
            metadata: FloppyMetadata::from([("title".to_string(), title.to_string())]),
            origtracktype: [Default::default(); FLOPPY_MAX_SIDES],
            dirty: false,
            force_wp: false,
            source_format: None,
        }
    }

    /// Sets the source format of this image (for save auto-detection)
    pub fn set_source_format(&mut self, format: ImageType) {
        self.source_format = Some(format);
    }

    /// Gets the source format of this image
    pub fn get_source_format(&self) -> Option<ImageType> {
        self.source_format
    }

    /// Resizes the length of a track to the actual size used in the image
    pub(crate) fn set_actual_track_length(&mut self, side: usize, track: usize, sz: usize) {
        let TrackLength::Bits(_old_sz) = self.get_track_length(side, track) else {
            panic!("Invalid operation on a flux track")
        };

        self.bitlen[side][track] = sz;
        let mut rng = rand::rng();
        self.trackdata[side][track].resize_with(sz / 8 + 1, || rng.random());
    }

    pub fn get_track_type(&self, side: usize, track: usize) -> TrackType {
        if !self.flux_trackdata[side][track].is_empty() {
            assert!(self.trackdata[side][track].is_empty());
            TrackType::Flux
        } else {
            assert!(self.flux_trackdata[side][track].is_empty());
            TrackType::Bitstream
        }
    }

    pub(crate) fn push_byte(&mut self, side: usize, track: usize, byte: u8) {
        assert!(self.flux_trackdata[side][track].is_empty());
        self.bitlen[side][track] += 8;
        self.trackdata[side][track].push(byte);
    }

    pub(crate) fn push_flux(&mut self, side: usize, track: usize, transition: FluxTicks) {
        self.trackdata[side][track].clear();
        self.flux_trackdata[side][track].push(transition);
    }

    pub(crate) fn set_metadata(&mut self, key: &str, val: &str) {
        self.metadata.insert(key.to_lowercase(), val.to_string());
    }

    /// Gets the original type of a track
    pub fn get_original_track_type(&self, side: usize, track: usize) -> OriginalTrackType {
        self.origtracktype[side][track]
    }

    /// Count the amount of tracks of a specific original track type
    pub fn count_original_track_type(&self, origtype: OriginalTrackType) -> usize {
        self.origtracktype
            .iter()
            .fold(0, |a, s| a + s.iter().filter(|&&t| t == origtype).count())
    }

    pub fn get_track_transition(&self, side: usize, track: usize, position: usize) -> FluxTicks {
        assert_eq!(self.get_track_type(side, track), TrackType::Flux);

        self.flux_trackdata[side][track][position]
    }

    /// Check if image was written to
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Forces floppy to be write-protected
    pub fn set_force_wp(&mut self) {
        self.force_wp = true;
    }

    pub(crate) fn push_track_bit(
        &mut self,
        side: usize,
        track: usize,
        position: usize,
        value: bool,
    ) {
        let byte = position / 8;
        let bit = 7 - position % 8;

        self.trackdata[side][track][byte] &= !(1 << bit);
        if value {
            self.trackdata[side][track][byte] |= 1 << bit;
        }
    }
}

impl Floppy for FloppyImage {
    fn get_type(&self) -> FloppyType {
        self.floppy_type
    }

    fn get_track_count(&self) -> usize {
        80
    }

    fn get_track_bit(&self, side: usize, track: usize, position: usize) -> bool {
        assert_eq!(self.get_track_type(side, track), TrackType::Bitstream);

        let byte = position / 8;
        let bit = 7 - position % 8;
        self.trackdata[side][track][byte] & (1 << bit) != 0
    }

    fn set_track_bit(&mut self, side: usize, track: usize, position: usize, value: bool) {
        self.push_track_bit(side, track, position, value);

        self.dirty = true;
    }

    fn get_track_length(&self, side: usize, track: usize) -> TrackLength {
        match self.get_track_type(side, track) {
            TrackType::Bitstream => TrackLength::Bits(self.bitlen[side][track]),
            TrackType::Flux => TrackLength::Transitions(self.flux_trackdata[side][track].len()),
        }
    }

    fn get_side_count(&self) -> usize {
        match self.floppy_type {
            FloppyType::Mac400K => 1,
            FloppyType::Mac800K => 2,
            FloppyType::Mfm144M => 2,
        }
    }

    fn get_title(&self) -> &str {
        &self.title
    }

    fn get_metadata(&self) -> FloppyMetadata {
        self.metadata.clone()
    }

    fn get_write_protect(&self) -> bool {
        // Forced write protection only - flux tracks can be converted on-demand
        self.force_wp
    }
}

impl FloppyImage {
    /// Converts a flux track to a bitstream track.
    /// This is needed when writing to a track that was originally loaded as flux.
    /// The flux data is decoded to bitstream using MFM timing.
    pub fn convert_flux_to_bitstream(&mut self, side: usize, track: usize) {
        if self.get_track_type(side, track) != TrackType::Flux {
            return;
        }

        // Take the flux data
        let flux_data = std::mem::take(&mut self.flux_trackdata[side][track]);
        if flux_data.is_empty() {
            return;
        }

        // Calculate expected track length for MFM
        let track_len = self.floppy_type.get_approx_track_length(track);

        // Initialize bitstream track with zeros
        self.bitlen[side][track] = track_len;
        self.trackdata[side][track] = vec![0u8; track_len / 8 + 1];

        // Decode flux transitions to bitstream using MFM timing
        // MFM at 300 RPM with 500kbps data rate = 2us per bit cell
        // With 8MHz base clock, that's 16 ticks per bit cell
        // Flux ticks are in units that vary by format, typically ~8MHz
        const TICKS_PER_BIT_CELL: i16 = 16;
        const HALF_CELL: i16 = TICKS_PER_BIT_CELL / 2;

        let mut bit_position = 0;
        let mut accumulated_ticks: i16 = 0;

        for transition in flux_data {
            accumulated_ticks += transition;

            // Determine how many bit cells this transition spans
            // Round to nearest bit cell count
            let bit_cells = ((accumulated_ticks + HALF_CELL) / TICKS_PER_BIT_CELL) as usize;

            if bit_cells > 0 {
                // Write zeros for intermediate bit cells
                for _ in 0..bit_cells.saturating_sub(1) {
                    if bit_position < track_len {
                        // Bit is already 0 from initialization
                        bit_position += 1;
                    }
                }
                // Write a 1 for the transition
                if bit_position < track_len {
                    self.push_track_bit(side, track, bit_position, true);
                    bit_position += 1;
                }

                accumulated_ticks -= (bit_cells as i16) * TICKS_PER_BIT_CELL;
            }
        }

        // Update the actual track length to what we wrote
        self.bitlen[side][track] = bit_position.min(track_len);
        self.origtracktype[side][track] = OriginalTrackType::Bitstream;
    }
}
