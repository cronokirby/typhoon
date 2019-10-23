//! This module contains core types for various concepts in Bittorrent.
//!
//! This includes definitions of things like piece hashes, peers, as well
//! as what's included in a `.torrent` file, for example.
use std::{convert::TryFrom, path::PathBuf, time::SystemTime};

pub enum TryFromBencodingError {}

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

/// Represents the SHA1 hash of a given piece.
///
/// This is how we verify the integrity of the data we receive from a torrent.
/// For each piece, we can calculate the SHA1 hash of that piece, and compare that
/// to the information we know about that torrent.
pub struct PieceHash([u8; 20]);

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
    pub creation: Option<SystemTime>,
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parsing_udp_trackers_works() {
        let tracker_string = "udp://tracker.leechers-paradise.org:6969";
        let expected = TrackerAddr::UDP("tracker.leechers-paradise.org:6969".to_owned());
        assert_eq!(expected, TrackerAddr::from(tracker_string));
    }
}
