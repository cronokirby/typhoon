use std::collections::HashMap;

/// Represents a general data structure expressable with "bencoding"
///
/// Bencoding has similar features to JSON, notably strings, integers,
/// lists/arrays, and key/value maps. This enum represents the raw data structure
/// of a bencoded file. We usually want to then inspect this general structure in order
/// to extract a more specific structure, such as information about a torrent.
///
/// Throughout the enum we choose `Box<[u8]>` instead of `Vec<u8>`
/// because it fits the semantics of our immutable representation better.
/// It's also slightly more efficient, since we avoid having to store an extra `capacity`
/// field for each string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bencoding {
    /// Represents an integer.
    ///
    /// Bencoding allows for negative integers, and we need to be able to represent
    /// the sizes of large files in the context of bittorrent: this means using `i64`.
    ///
    /// Eventually, we may want to narrow this down to `u64` to eliminate things like
    /// negative file sizes, but in general bencoding allows negative integers.
    Int(i64),
    /// Represents a sequence of bytes.
    ///
    /// Bencoding does not impose any character encodings on strings, but UTF-8 is used
    /// in practice for human-readable strings. However, many bencoded files make use of
    /// strings that are **not human-readable** and **not UTF-8**. For example, torrent files
    /// contain SHA-1 hashes, which are just a sequence of bytes.
    ByteString(Box<[u8]>),
    /// Represents an ordered sequence of bencoded elements.
    List(Box<[Bencoding]>),
    /// Represents a mapping from byte sequences to bencoded elements.
    ///
    /// The keys of this map are subject to the same caveats as byte sequence elements in this
    /// enum. In practice though, non UTF-8 map keys don't seem to appear.
    Dict(HashMap<Box<[u8]>, Bencoding>),
}
