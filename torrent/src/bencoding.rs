use std::{collections::HashMap, path::Path};

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_until},
    combinator::{map, map_res},
    multi::{length_value, many0, many1},
    sequence::{delimited, pair, preceded},
    Finish, IResult,
};

/// Represents a single BEncode item
#[derive(Debug, PartialEq, Clone)]
pub enum Item {
    ByteArray(Vec<u8>),
    Integer(usize),
    Dictionary(HashMap<String, Item>),
    List(Vec<Item>),
}

/// Represents an entire parsed BEncode snippet
#[derive(Debug)]
pub struct BEncoding {
    items: Vec<Item>,
}

impl BEncoding {
    /// Start code for dictionary
    const DICT_START: &str = "d";
    /// Start code for list
    const LIST_START: &str = "l";
    /// Start code for number
    const NUMBER_START: &str = "i";
    /// General end code
    const END: &str = "e";
    /// Seperator for byte array
    const ARRAY_SEP: &str = ":";

    /// Decodes a byte array, returning None if invalid bencone
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        Some(Self {
            items: parse_bytes(bytes).ok()?,
        })
    }

    /// Decodes a BEnconde string by first converting to a byte array
    pub fn decode_str(data: &str) -> Option<Self> {
        Self::decode(data.as_bytes())
    }

    /// Decodes a BEnconde file by first reading to a byte buffer and then decoding
    pub fn decode_path(path: impl AsRef<Path>) -> Option<Self> {
        let data = std::fs::read(path).ok()?;

        Self::decode(&data)
    }
}

/// Parse a single BEncoded integer of the form `i<number>e`
fn parse_integer(input: &[u8]) -> IResult<&[u8], usize> {
    map_res(
        map_res(
            delimited(
                tag(BEncoding::NUMBER_START),
                take_until(BEncoding::END),
                tag(BEncoding::END),
            ),
            std::str::from_utf8,
        ),
        |string: &str| string.parse(),
    )(input)
}

/// Parse a single BEncoded byte array of the form `<length>:<data>`
fn parse_bytearray(input: &[u8]) -> IResult<&[u8], &[u8]> {
    length_value(
        map(
            nom::character::complete::u32,
            |x| if x > 0 { x + 1 } else { 0 },
        ),
        preceded(tag(BEncoding::ARRAY_SEP), is_not("\0")),
    )(input)
}

/// Parse a BENcoded list of the form `l<element>*e`
fn parse_list(input: &[u8]) -> IResult<&[u8], Vec<Item>> {
    delimited(
        tag(BEncoding::LIST_START),
        many0(parse_item),
        tag(BEncoding::END),
    )(input)
}

/// Parse a BENcoded dict of the form `d(<element key><element value>)*e`
fn parse_dictionary(input: &[u8]) -> IResult<&[u8], HashMap<String, Item>> {
    map_res(
        delimited(
            tag(BEncoding::DICT_START),
            many0(pair(parse_bytearray, parse_item)),
            tag(BEncoding::END),
        ),
        |a| {
            a.iter()
                .map(|(key, value)| {
                    println!("{key:?} {value:?}");
                    std::str::from_utf8(key).map(|key| (key.to_owned(), value.clone()))
                })
                .collect()
        },
    )(input)
}

/// Parse any BEncoded item
fn parse_item(input: &[u8]) -> IResult<&[u8], Item> {
    alt((
        map(parse_integer, Item::Integer),
        map(parse_list, Item::List),
        map(parse_dictionary, Item::Dictionary),
        map(parse_bytearray, |slice| Item::ByteArray(slice.to_owned())),
    ))(input)
}

/// Parse a byte stream
fn parse_bytes(input: &[u8]) -> Result<Vec<Item>, nom::error::Error<&[u8]>> {
    many1(parse_item)(input)
        .finish()
        .map(|(_remaining, items)| items)
}

#[cfg(test)]
mod test {
    use super::*;
    use nom_test_helpers::{
        assert_done_and_eq, assert_error, assert_finished, assert_finished_and_eq,
    };

    #[test]
    fn test_number_parser() {}

    #[test]
    fn test_bytearray_parser() {
        assert_finished_and_eq!(parse_bytearray(b"4:spam"), b"spam");
        assert_finished_and_eq!(parse_bytearray(b"5:sp am"), b"sp am");
        assert_done_and_eq!(parse_bytearray(b"2:spam"), b"sp");
        assert_error!(parse_bytearray(b"10:aa"));
    }

    #[test]
    fn test_list_parser() {
        assert_finished_and_eq!(
            parse_list(b"l4:spam4:eggse"),
            vec![
                Item::ByteArray(b"spam".to_vec()),
                Item::ByteArray(b"eggs".to_vec())
            ]
        );

        assert_finished_and_eq!(
            parse_list(b"l4:spami10ee"),
            vec![Item::ByteArray(b"spam".to_vec()), Item::Integer(10)]
        );
    }

    #[test]
    fn test_dict_parser() {
        assert_finished_and_eq!(
            parse_dictionary(b"d3:cow3:moo4:spam4:eggse"),
            HashMap::from([
                ("cow".to_owned(), Item::ByteArray(b"moo".to_vec())),
                ("spam".to_owned(), Item::ByteArray(b"eggs".to_vec()))
            ])
        );

        assert_finished_and_eq!(
            parse_dictionary(b"d4:spaml1:a1:bee"),
            HashMap::from([(
                "spam".to_owned(),
                Item::List(vec![
                    Item::ByteArray(b"a".to_vec()),
                    Item::ByteArray(b"b".to_vec())
                ])
            ),])
        );

        assert_finished_and_eq!(
            parse_dictionary(b"d4:infod6:lengthi20eee"),
            HashMap::from([(
                "info".to_owned(),
                Item::Dictionary(HashMap::from([("length".to_owned(), Item::Integer(20)),]))
            ),])
        );
    }

    #[test]
    fn test_total_parser() {
        assert!(BEncoding::decode_path("../sample.torrent").is_some());
        assert!(BEncoding::decode_path("../archlinux-2022.10.01-x86_64.iso.torrent").is_some());
    }
}
