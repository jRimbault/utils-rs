use crate::And;

use super::{Assignment, Connection, LogicGate};
use std::error;
use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionParseError {
    InvalidFormat,
    InvalidGate,
    InvalidInput,
    InvalidInt(ParseIntError),
}

impl From<ParseIntError> for ConnectionParseError {
    fn from(value: ParseIntError) -> Self {
        Self::InvalidInt(value)
    }
}

impl fmt::Display for ConnectionParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnectionParseError::InvalidFormat => f.write_str("Invalid format"),
            ConnectionParseError::InvalidGate => f.write_str("Invalid gate"),
            ConnectionParseError::InvalidInput => f.write_str("Invalid input"),
            ConnectionParseError::InvalidInt(e) => fmt::Display::fmt(&e, f),
        }
    }
}

impl error::Error for ConnectionParseError {}

impl FromStr for LogicGate {
    type Err = ConnectionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.split_whitespace();
        let gate = match (s.next(), s.next(), s.next()) {
            (Some(part0), None, None) => match part0.parse::<u16>() {
                Ok(value) => LogicGate::Assignment(Assignment::Direct(value)),
                _ => LogicGate::Assignment(Assignment::Gate(part0.to_string())),
            },
            (Some(part0), Some(part1), None) if part0 == "NOT" => LogicGate::Not(part1.to_owned()),
            (Some(part0), Some(part1), Some(part2)) => {
                let (origin, gate, dest) = (part0, part1, part2);
                let origin = origin.to_owned();
                match gate {
                    "AND" => {
                        if origin == "1" {
                            LogicGate::And(And::True(dest.to_owned()))
                        } else {
                            LogicGate::And(And::And(origin, dest.to_owned()))
                        }
                    }
                    "OR" => LogicGate::Or(origin, dest.to_owned()),
                    "LSHIFT" => LogicGate::Lshift(origin, dest.parse()?),
                    "RSHIFT" => LogicGate::Rshift(origin, dest.parse()?),
                    _ => return Err(ConnectionParseError::InvalidFormat),
                }
            }
            _ => return Err(ConnectionParseError::InvalidFormat),
        };
        Ok(gate)
    }
}

impl FromStr for Connection {
    type Err = ConnectionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(" -> ");
        match (parts.next(), parts.next(), parts.next()) {
            (Some(part0), Some(part1), None) => {
                let output = part1.to_string();
                let gate: LogicGate = part0.parse()?;
                Ok(Connection { gate, output })
            }
            _ => Err(ConnectionParseError::InvalidFormat),
        }
    }
}
