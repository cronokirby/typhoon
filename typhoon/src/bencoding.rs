extern crate nom;
use nom::{
    branch::alt,
    bytes::complete::take_while1,
    character::{complete::char, is_digit},
    combinator::{map, map_res},
    sequence::{preceded, terminated},
    IResult,
};

use std::collections::HashMap;
use std::str;

/// Represents an error that occurs while parsing bencoded data.
///
/// For now, this isn't very useful, and just contains a formatted string
/// produced by our parsing framework. We could produce more useful input by
/// manually inspecting errors to figure out how exactly things failed, or if
/// we could re run the parser on the data with invalid UTF-8 sections stripped.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BencodingError(String);

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

impl Bencoding {
    fn parse(input: &[u8]) -> Result<Bencoding, BencodingError> {
        match bencoding(input) {
            Ok((_, res)) => Ok(res),
            Err(e) => {
                let msg = match e {
                    nom::Err::Error((raw, kind)) | nom::Err::Failure((raw, kind)) => {
                        let lossy = String::from_utf8_lossy(raw);
                        format!("lossy: {}, raw: {:?}, kind: {:?}", lossy, raw, kind)
                    }
                    other => format!("{:?}", other),
                };
                Err(BencodingError(msg))
            }
        }
    }
}

fn bencoding(input: &[u8]) -> IResult<&[u8], Bencoding> {
    // The return value of this function is always positive
    fn int_digits(input: &[u8]) -> IResult<&[u8], i64> {
        let digits = take_while1(is_digit);
        // We don't need to check, since we've parse only ASCII digits
        let get_str = map(digits, |bytes| unsafe { str::from_utf8_unchecked(bytes) });
        // This shouldn't ever fail, once again because of the ASCII digits
        map_res(get_str, |string| i64::from_str_radix(string, 10))(input)
    }

    fn signed_int(input: &[u8]) -> IResult<&[u8], i64> {
        let negative = map(preceded(char('-'), int_digits), |i| -i);
        alt((negative, int_digits))(input)
    }

    fn int(input: &[u8]) -> IResult<&[u8], Bencoding> {
        let wrapped = terminated(preceded(char('i'), signed_int), char('e'));
        map(wrapped, Bencoding::Int)(input)
    }

    int(input)
}

#[cfg(test)]
mod test {
    use super::Bencoding;

    #[test]
    fn parsing_positive_integers_works() {
        let input = b"i123e";
        let output = Bencoding::parse(input);
        assert_eq!(Ok(Bencoding::Int(123)), output);
    }

    #[test]
    fn parsing_negative_integers_works() {
        let input = b"i-111e";
        let output = Bencoding::parse(input);
        assert_eq!(Ok(Bencoding::Int(-111)), output);
    }
}
