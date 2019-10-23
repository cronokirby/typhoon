//! This module contains core types for various concepts in Bittorrent.
//!
//! This includes definitions of things like piece hashes, peers, as well
//! as what's included in a `.torrent` file, for example.
use crate::bencoding::Bencoding;
use std::{convert::TryFrom, path::PathBuf, str, time};

pub enum TryFromBencodingError<'b> {
    ExpectedInt(&'b Bencoding),
    ExpectedByteString(&'b Bencoding),
    ExpectedList(&'b Bencoding),
    ExpectedDict(&'b Bencoding),
    ExceedsSystemTime(i64),
    NotUTF8 {
        bencoding: &'b Bencoding,
        error: str::Utf8Error,
    },
    MissingKey {
        bencoding: &'b Bencoding,
        key: &'static [u8],
    },
}

impl<'b> TryFromBencodingError<'b> {
    fn from_utf8_error(bencoding: &'b Bencoding, error: str::Utf8Error) -> Self {
        TryFromBencodingError::NotUTF8 { bencoding, error }
    }
}

#[inline]
fn extract_int<'b>(bencoding: &'b Bencoding) -> Result<i64, TryFromBencodingError<'b>> {
    match bencoding {
        &Bencoding::Int(i) => Ok(i),
        _ => Err(TryFromBencodingError::ExpectedInt(bencoding)),
    }
}

#[inline]
fn extract_bytes<'b>(bencoding: &'b Bencoding) -> Result<&'b [u8], TryFromBencodingError<'b>> {
    match bencoding {
        Bencoding::ByteString(bx) => Ok(bx),
        _ => Err(TryFromBencodingError::ExpectedByteString(bencoding)),
    }
}

#[inline]
fn extract_string<'b>(bencoding: &'b Bencoding) -> Result<&'b str, TryFromBencodingError<'b>> {
    let bytes = extract_bytes(bencoding)?;
    str::from_utf8(bytes).map_err(|e| TryFromBencodingError::from_utf8_error(bencoding, e))
}

#[inline]
fn extract_key<'b>(
    bencoding: &'b Bencoding,
    key: &'static [u8],
) -> Result<&'b Bencoding, TryFromBencodingError<'b>> {
    match bencoding {
        Bencoding::Dict(map) => map
            .get(key)
            .ok_or(TryFromBencodingError::MissingKey { bencoding, key }),
        _ => Err(TryFromBencodingError::ExpectedDict(bencoding)),
    }
}

#[inline]
fn extract_list<'b>(
    bencoding: &'b Bencoding,
) -> Result<&'b [Bencoding], TryFromBencodingError<'b>> {
    match bencoding {
        Bencoding::List(bx) => Ok(bx),
        _ => Err(TryFromBencodingError::ExpectedList(bencoding)),
    }
}

#[inline]
fn extract_system_time<'b>(
    bencoding: &'b Bencoding,
) -> Result<time::SystemTime, TryFromBencodingError<'b>> {
    let seconds = extract_int(bencoding)?;
    let from_beginning = time::Duration::from_secs(seconds as u64);
    time::UNIX_EPOCH
        .checked_add(from_beginning)
        .ok_or(TryFromBencodingError::ExceedsSystemTime(seconds))
}

/// Represents the location of some tracker.
///
/// Trackers are how we bootstrap into an existing swarm. We need to
/// know a list of IPs for peers downloading the same file as us. We connect
/// to a tracker and ask it for this information.
///
/// Addresses are kept as strings, because they often require some kind of DNS
/// resolution, e.g. "tracker.leechers-paradise.org:6969".
#[derive(Clone, Debug, PartialEq)]
pub enum TrackerAddr {
    /// An address of a tracker that speaks the UDP protocol.
    ///
    /// The UDP based tracker protocol is quite a bit more common, since it's more
    /// efficient than the HTTP based protocol.
    UDP(String),
    /// An HTTP or HTTPS based tracker.
    ///
    /// This variant will include the protocol qualified url (e.g. "https://tracker.com:4040").
    /// We include this to be able to let our HTTP client distinguish between the two protocols.
    HTTP(String),
    /// This covers other protocols we don't support or recognize.
    ///
    /// The main protocol included in here is websocket trackers, used
    /// to allow torrents on the web.
    Unknown(String),
}

impl From<&str> for TrackerAddr {
    fn from(string: &str) -> Self {
        let maybe_udp = string.splitn(2, "udp://").skip(1).next();
        if let Some(udp) = maybe_udp {
            return TrackerAddr::UDP(udp.to_owned());
        }
        if string.starts_with("http://") || string.starts_with("https://") {
            // We include the entire string, because http clients like having the URL
            return TrackerAddr::HTTP(string.to_owned());
        }
        return TrackerAddr::Unknown(string.to_owned());
    }
}

impl<'b> TryFrom<&'b Bencoding> for TrackerAddr {
    type Error = TryFromBencodingError<'b>;

    fn try_from(bencoding: &'b Bencoding) -> Result<Self, Self::Error> {
        extract_string(bencoding).map(Self::from)
    }
}

const PIECE_HASH_SIZE: usize = 20;

/// Represents the SHA1 hash of a given piece.
///
/// This is how we verify the integrity of the data we receive from a torrent.
/// For each piece, we can calculate the SHA1 hash of that piece, and compare that
/// to the information we know about that torrent.
pub struct PieceHash([u8; PIECE_HASH_SIZE]);

/// This contains the info about a specific file in this torrent.
///
/// Torrents include multiple files, each of which has a full path, and a given length.
///
/// For example, a movie might have a main file `movie.mp4` as well as subtitles
/// `subtitles/it.srt`, `subtitles/en.srt`. The video file will be quite a bit larger than
/// the subtitles, of course.
pub struct FileInfo {
    /// This holds the path of the file.
    pub name: PathBuf,
    /// How many bytes does this file contain.
    pub length: usize,
}

/// Represents the information contained in a .torrent file.
///
/// This includes information about the files contained in a torrent, including
/// how they're divided up into pieces, as well as how to connect to an existing
/// swarm for this torrent.
pub struct Torrent {
    /// A list of trackers we can connect to, with different priorities.
    ///
    /// The first element of each tuple represents the priority of that tracker,
    /// with lower values needing to be tried first. The idea is to try trackers
    /// one by one, only moving on to the next if we fail to get a response. We can try
    /// trackers of the same priority in any order, but lower values should be tried before
    /// higher values.
    pub trackers: Box<[(u8, TrackerAddr)]>,
    /// If present, this contains the time of creation of this torrent.
    pub creation: Option<time::SystemTime>,
    /// If present, this contains a message about this torrent.
    pub comment: Option<String>,
    /// If present, this contains a description of the program that created this torrent.
    pub created_by: Option<String>,
    /// Whether or not this torrent is private.
    ///
    /// Private torrents are made to avoid letting just anyone join the swarm for that file.
    /// The way this works is by having trackers simply not respond to unrecognized users.
    /// For this mechanism to work, we need to not circumvent it by finding peers through other
    /// means than trackers, such as DHT, or PEX.
    ///
    /// For private torrents, we are not allowed to find or broadcast to new peers besides communicating
    /// with the trackers listed in this torrent file.
    pub private: bool,
    /// How many bytes are in each piece (except for the last one).
    pub piece_length: usize,
    /// A sequence of hashes, for each piece in the torrent.
    ///
    /// This is what allows us to verify the integrity of the torrent as a whole.
    /// Whenever we download a new piece, we can hash its contents, and compare it to the
    /// corresponding hash contained here.
    pub piece_hashes: Box<[PieceHash]>,
    /// This contained a sequence of information about the files in this torrent.
    ///
    /// Torrents usually contain multiple files, and we need to be able to handle that.
    /// The way pieces are distributed among files is simple. The files are concatenated
    /// and considered as a big byte array. Pieces are then distributed along this array.
    /// This means that a piece can overlap an arbitrary number of files, and that the final
    /// piece may be a different length than the others.
    pub files: Box<[FileInfo]>,
}

pub enum ParseTorrentError<'b> {
    Bencoding(TryFromBencodingError<'b>),
    BadHashLength(usize),
}

impl<'b> From<TryFromBencodingError<'b>> for ParseTorrentError<'b> {
    fn from(error: TryFromBencodingError<'b>) -> Self {
        ParseTorrentError::Bencoding(error)
    }
}

impl<'b> TryFrom<&'b Bencoding> for Torrent {
    type Error = ParseTorrentError<'b>;

    fn try_from(bencoding: &'b Bencoding) -> Result<Self, Self::Error> {
        let trackers = match extract_key(bencoding, b"announce-list") {
            Err(_) => {
                let tracker = TrackerAddr::try_from(bencoding)?;
                vec![(0, tracker)].into_boxed_slice()
            }
            Ok(inner) => {
                let tiers = extract_list(inner)?;
                let mut trackers = Vec::with_capacity(tiers.len());
                for (index, tier) in tiers.iter().enumerate() {
                    let tier_list = extract_list(tier)?;
                    for tracker in tier_list {
                        trackers.push((index as u8, TrackerAddr::try_from(tracker)?))
                    }
                }
                trackers.into_boxed_slice()
            }
        };
        let creation = extract_key(bencoding, b"creation date")
            .ok()
            .map(extract_system_time)
            .transpose()?;
        let comment = extract_key(bencoding, b"comment")
            .ok()
            .map(|inner| extract_string(inner).map(String::from))
            .transpose()?;
        let created_by = extract_key(bencoding, b"created by")
            .ok()
            .map(|inner| extract_string(inner).map(String::from))
            .transpose()?;
        let info = extract_key(bencoding, b"info")?;
        let private_option = extract_key(info, b"private")
            .ok()
            .map(extract_int)
            .transpose()?;
        let private = private_option.map(|x| x == 1).unwrap_or(false);
        let piece_length = extract_int(extract_key(info, b"piece length")?)? as usize;
        let piece_bytes = extract_bytes(extract_key(info, b"pieces")?)?;
        let piece_bytes_len = piece_bytes.len();
        if piece_bytes_len % PIECE_HASH_SIZE != 0 {
            return Err(ParseTorrentError::BadHashLength(piece_bytes_len));
        }
        let mut piece_hashes = Vec::with_capacity(piece_bytes_len / PIECE_HASH_SIZE);
        for chunk in piece_bytes.chunks_exact(PIECE_HASH_SIZE) {
            let mut arr: [u8; PIECE_HASH_SIZE] = Default::default();
            arr.copy_from_slice(chunk);
            piece_hashes.push(PieceHash(arr));
        }
        let piece_hashes = piece_hashes.into_boxed_slice();
        let files = match extract_key(info, b"files") {
            Err(_) => {
                let name: PathBuf = extract_string(extract_key(info, b"name")?)?.into();
                let length = extract_int(extract_key(info, b"length")?)? as usize;
                vec![FileInfo { name, length }].into_boxed_slice()
            }
            Ok(inner) => {
                let dir: PathBuf = extract_string(extract_key(info, b"name")?)?.into();
                let files = extract_list(inner)?;
                let mut file_infos = Vec::with_capacity(files.len());
                for file in files {
                    let mut name = dir.clone();
                    let length = extract_int(extract_key(file, b"length")?)? as usize;
                    let path: PathBuf = extract_string(extract_key(file, b"path")?)?.into();
                    name.push(path);
                    file_infos.push(FileInfo { name, length });
                }
                file_infos.into_boxed_slice()
            }
        };
        Ok(Torrent {
            trackers,
            creation,
            comment,
            created_by,
            private,
            piece_length,
            piece_hashes,
            files,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parsing_udp_tracker_addrs() {
        let tracker_string = "udp://tracker.leechers-paradise.org:6969";
        let expected = TrackerAddr::UDP("tracker.leechers-paradise.org:6969".to_owned());
        assert_eq!(expected, TrackerAddr::from(tracker_string));
    }

    #[test]
    fn parsing_http_tracker_addrs() {
        let tracker_string = "http://tracker.leechers-paradise.org:6969";
        let expected = TrackerAddr::HTTP("http://tracker.leechers-paradise.org:6969".to_owned());
        assert_eq!(expected, TrackerAddr::from(tracker_string));
    }
}
