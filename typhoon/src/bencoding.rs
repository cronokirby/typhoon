use std::collections::HashMap;

/// Represents an error that occurs while parsing bencoded data.
///
/// For now, this isn't very useful, and just contains a formatted string
/// produced by our parsing framework. We could produce more useful input by
/// manually inspecting errors to figure out how exactly things failed, or if
/// we could re run the parser on the data with invalid UTF-8 sections stripped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BencodingError(String);

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

/// A type synonym for the result of parsing bencoded data.
pub type BencodingResult = Result<Bencoding, BencodingError>;

impl Bencoding {
    pub fn parse(input: &[u8]) -> BencodingResult {
        fn int_digits(lexer: &mut Lexer) -> Result<i64, BencodingError> {
            let head = *lexer.peek().ok_or(BencodingError(
                "Tried to parse integer from empty input".to_owned(),
            ))?;
            let mut acc = as_digit(head).ok_or(BencodingError(
                "Tried to parse integer without any valid digits".to_owned(),
            ))?;
            lexer.next();
            while let Some(&chr) = lexer.peek() {
                match as_digit(chr) {
                    None => break,
                    Some(digit) => {
                        lexer.next();
                        acc = 10 * acc + digit;
                    }
                }
            }
            Ok(acc)
        }

        fn int(lexer: &mut Lexer) -> BencodingResult {
            let negate = if let Some(b'-') = lexer.peek() {
                lexer.next();
                -1
            } else {
                1
            };
            let int = int_digits(lexer)?;
            lexer.expect(b'e')?;
            Ok(Bencoding::Int(negate * int))
        }

        fn bytestring(lexer: &mut Lexer) -> Result<Box<[u8]>, BencodingError> {
            let count = int_digits(lexer)? as usize;
            lexer.expect(b':')?;
            let slice = lexer.take(count).ok_or(BencodingError(format!(
                "Unable to take {} bytes from input",
                count
            )))?;
            Ok(slice.to_vec().into_boxed_slice())
        }

        fn list(lexer: &mut Lexer) -> BencodingResult {
            let mut inner = Vec::new();
            while let Ok(item) = root(lexer) {
                inner.push(item);
            }
            lexer.expect(b'e')?;
            Ok(Bencoding::List(inner.into_boxed_slice()))
        }

        fn dict(lexer: &mut Lexer) -> BencodingResult {
            let mut inner = HashMap::new();
            while let Ok(key) = bytestring(lexer) {
                let item = root(lexer)?;
                inner.insert(key, item);
            }
            lexer.expect(b'e')?;
            Ok(Bencoding::Dict(inner))
        }

        fn root(lexer: &mut Lexer) -> BencodingResult {
            match lexer.peek() {
                None => Err(BencodingError(
                    "Tried to parse bencoded data from empty input".to_owned(),
                )),
                Some(b'i') => {
                    lexer.next();
                    int(lexer)
                }
                Some(b'l') => {
                    lexer.next();
                    list(lexer)
                }
                Some(b'd') => {
                    lexer.next();
                    dict(lexer)
                }
                Some(&c) if as_digit(c).is_some() => bytestring(lexer).map(Bencoding::ByteString),
                Some(c) => Err(BencodingError(format!("Unknown type of element {}", c))),
            }
        }

        let mut lexer = Lexer::new(input);
        root(&mut lexer)
    }
}

#[derive(Debug)]
struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    #[inline]
    fn new(input: &'a [u8]) -> Self {
        Lexer { input, pos: 0 }
    }

    #[inline]
    fn next(&mut self) -> Option<&'a u8> {
        let ret = self.input.get(self.pos);
        self.pos += 1;
        ret
    }

    #[inline]
    fn peek(&mut self) -> Option<&'a u8> {
        self.input.get(self.pos)
    }

    #[inline]
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let top = self.pos + count;
        if top > self.input.len() {
            None
        } else {
            let slice = &self.input[self.pos..top];
            self.pos = top;
            Some(slice)
        }
    }

    #[inline]
    fn expect(&mut self, target: u8) -> Result<(), BencodingError> {
        match self.peek() {
            Some(&good) if good == target => {
                self.next();
                Ok(())
            }
            Some(bad) => Err(BencodingError(format!(
                "Expected {} but found {}",
                target, bad
            ))),
            None => Err(BencodingError(format!(
                "Expected {} but reached the end of input",
                target
            ))),
        }
    }
}
// Check that an ASCII character is between '0' and '9'
fn as_digit(chr: u8) -> Option<i64> {
    if b'0' <= chr && chr <= b'9' {
        Some(chr as i64 - 48)
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::{as_digit, Bencoding};

    #[test]
    fn as_digit_test() {
        assert_eq!(Some(1), as_digit(b'1'))
    }

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

    #[test]
    fn parsing_basic_strings_works() {
        let input = b"4:AAAA";
        let output = Bencoding::parse(input);
        let string = b"AAAA".to_vec().into_boxed_slice();
        assert_eq!(Ok(Bencoding::ByteString(string)), output);
    }

    #[test]
    fn parsing_basic_lists_works() {
        let input = b"li1ei2ei3ee";
        let output = Bencoding::parse(input);
        let expected = Bencoding::List(Box::new([
            Bencoding::Int(1),
            Bencoding::Int(2),
            Bencoding::Int(3),
        ]));
        assert_eq!(Ok(expected), output);
    }

    #[test]
    fn parsing_basic_dicts_works() {
        let input = b"d1:Ai1e1:Bi2ee";
        let output = Bencoding::parse(input);
        let mut map = HashMap::new();
        map.insert(b"A".to_vec().into_boxed_slice(), Bencoding::Int(1));
        map.insert(b"B".to_vec().into_boxed_slice(), Bencoding::Int(2));
        let expected = Bencoding::Dict(map);
        assert_eq!(Ok(expected), output);
    }
}
