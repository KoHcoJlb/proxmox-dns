use std::collections::BTreeMap;

use macaddr::MacAddr6;
use nom::{
    AsChar,
    bytes::complete::{tag, take_while_m_n},
    character::complete::anychar,
    combinator::map,
    IResult,
    multi::{many_m_n, many_till},
    sequence::{preceded, tuple},
};
use serde::{Deserializer, Serializer};
use serde_with::{
    As,
    DisplayFromStr,
    FromInto,
    FromIntoRef,
    with_prefix::WithPrefix,
};

pub trait Prefix {
    const PREFIX: &'static str;
}

#[derive(Debug)]
pub struct Spec<T>(pub String, T);

impl<T: Default> From<String> for Spec<T> {
    fn from(value: String) -> Self {
        Self(value, Default::default())
    }
}

impl<T> From<&Spec<T>> for String {
    fn from(value: &Spec<T>) -> Self {
        value.0.clone()
    }
}

impl<T> Spec<T> where T: Default, Spec<T>: Prefix {
    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<u32, Self>, D::Error>
    where D: Deserializer<'de> {
        As::<BTreeMap<DisplayFromStr, FromInto<String>>>::deserialize(WithPrefix {
            delegate: deserializer,
            prefix: Self::PREFIX,
        })
    }

    pub fn serialize<S>(t: &BTreeMap<u32, Self>, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        As::<BTreeMap<DisplayFromStr, FromIntoRef<String>>>::serialize(t, WithPrefix {
            delegate: serializer,
            prefix: Self::PREFIX,
        })
    }
}

fn mac_address(s: &str) -> IResult<&str, MacAddr6, nom::error::Error<&str>> {
    let octet = |s| map(
        take_while_m_n(2, 2, |c: char| c.is_hex_digit()),
        |o| u8::from_str_radix(o, 16).unwrap(),
    )(s);

    let (input, (first, mut octets)) = tuple((
        octet,
        many_m_n(5, 5, preceded(tag(":"), octet))
    ))(s)?;

    octets.insert(0, first);
    Ok((input, MacAddr6::from(TryInto::<[u8; 6]>::try_into(octets).unwrap())))
}

impl<T> Spec<T> {
    pub fn extract_mac(&self) -> super::Result<MacAddr6> {
        let (_, (_, mac)) = many_till(anychar, mac_address)(&self.0)
            .map_err(|_| super::Error::Parse)?;
        Ok(mac)
    }
}
