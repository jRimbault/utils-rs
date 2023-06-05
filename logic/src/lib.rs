//! Library attempting to model a logic circuit.
//!
//! The main type of this library is [`Circuit`] which can be built
//! from a collections of [`Connection`]s, eg:
//!
//! ```
//! # use logic::{Assignment, Circuit, Connection, LogicGate};
//! let circuit = Circuit::from_iter([
//!     Connection::new("a", LogicGate::Assignment(Assignment::Direct(16)))
//! ]);
//! assert_eq!(circuit.signal("a"), Some(16));
//! ```
//!
//! A [`Circuit`] can also be built from a file or a string, which it will attempts to parse
//! as a series of [`Connection`]s line-by-line.
//!
//! ```
//! # use logic::Circuit;
//! # let path: &std::path::Path = "tests/small.txt".as_ref();
//! let circuit = Circuit::from_file(path).unwrap();
//! // or
//! let file = std::fs::read_to_string(path).unwrap();
//! let circuit = Circuit::from_string(&file).unwrap();
//! ```
//!
//! [`Connection`]s can be added to a [`Circuit`] after it has been built:
//!
//! ```
//! # use logic::{Assignment, Circuit, Connection, LogicGate};
//! let mut circuit = Circuit::default();
//! circuit.add_connection("16 -> a").unwrap();
//! assert_eq!(circuit.signal("a"), Some(16));
//! circuit.add_connection(Connection::new("b", LogicGate::Lshift("a".into(), 2))).unwrap();
//! assert_eq!(circuit.signal("b"), Some(64));
//! ```
mod connection;
mod signal;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};

pub use connection::ConnectionParseError;

#[derive(Debug, Clone)]
pub enum LogicGate {
    Assignment(Assignment),
    And(And),
    Or(String, String),
    Not(String),
    Lshift(String, u16),
    Rshift(String, u16),
}

#[derive(Debug, Clone)]
pub enum Assignment {
    Direct(u16),
    Gate(String),
}

#[derive(Debug, Clone)]
pub enum And {
    True(String),
    And(String, String),
}

#[derive(Debug, Clone)]
pub struct Connection {
    gate: LogicGate,
    output: String,
}

/// Type representing a logic circuit.
#[derive(Debug, Default)]
pub struct Circuit {
    connections: HashMap<String, Connection>,
}

/// Cached version of the logic circuit.
///
/// Use if you want better time performance at the cost of memory.
///
/// Can be built from a [`Circuit`]:
/// ```
/// # use logic::{CachedCircuit, Circuit};
/// let circuit = CachedCircuit::from(Circuit::default());
/// ```
#[derive(Debug, Default)]
pub struct CachedCircuit {
    circuit: Circuit,
    cache: RefCell<HashMap<String, u16>>,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Parse(ConnectionParseError),
}

impl Circuit {
    /// Attempts to build a `Circuit` from any kind of well-structured `Reader`.
    pub fn from_read<R>(reader: R) -> Result<Self, Error>
    where
        R: Read,
    {
        Self::from_bufread(BufReader::new(reader))
    }

    /// Attempts to build a `Circuit` from a string.
    pub fn from_string(s: &str) -> Result<Self, Error> {
        Self::from_bufread(s.as_bytes())
    }

    /// Convenience method to build a [`Circuit`] from a file.
    pub fn from_file(file_path: &std::path::Path) -> Result<Self, Error> {
        Self::from_read(File::open(file_path)?)
    }

    fn from_bufread<R>(reader: R) -> Result<Self, Error>
    where
        R: BufRead,
    {
        let mut circuit = Circuit::default();
        for line in reader.lines() {
            circuit.add_connection(&line?)?;
        }
        Ok(circuit)
    }

    /// Adds a new connection to the existing circuit.
    pub fn add_connection<T>(&mut self, connection: T) -> Result<(), ConnectionParseError>
    where
        T: TryIntoConnection,
    {
        let connection = connection.try_into_connection()?;
        self.connections
            .insert(connection.output.clone(), connection);
        Ok(())
    }

    /// Get the signal out of the specified cable. `None` if no matching cable.
    pub fn signal(&self, cable: &str) -> Option<u16> {
        let connection = self.connections.get(cable)?;
        let signal = signal::explore_gates(self, connection)?;
        Some(signal)
    }
}

impl CachedCircuit {
    /// Adds a new connection to the existing circuit.
    /// Same as [`Circuit::add_connection`], but resets the interval value cache.
    pub fn add_connection<T>(&mut self, connection: T) -> Result<(), ConnectionParseError>
    where
        T: TryIntoConnection,
    {
        self.circuit.add_connection(connection)?;
        self.cache.borrow_mut().clear();
        Ok(())
    }

    /// Get the signal out of the specified cable. `None` if no matching cable.
    pub fn signal(&self, cable: &str) -> Option<u16> {
        if let Some(i) = self.cache.borrow().get(cable) {
            return Some(*i);
        }
        let connection = self.circuit.connections.get(cable)?.clone();
        let signal = signal::explore_gates(self, &connection)?;
        self.cache.borrow_mut().insert(cable.into(), signal);
        Some(signal)
    }
}

impl From<Circuit> for CachedCircuit {
    fn from(circuit: Circuit) -> Self {
        Self {
            circuit,
            cache: Default::default(),
        }
    }
}

impl Connection {
    pub fn new(output: &str, gate: LogicGate) -> Self {
        Self {
            gate,
            output: output.into(),
        }
    }
}

pub trait TryIntoConnection {
    fn try_into_connection(self) -> Result<Connection, ConnectionParseError>;
}

impl TryIntoConnection for &str {
    fn try_into_connection(self) -> Result<Connection, ConnectionParseError> {
        self.parse()
    }
}

impl TryIntoConnection for &String {
    fn try_into_connection(self) -> Result<Connection, ConnectionParseError> {
        self.parse()
    }
}

impl TryIntoConnection for Connection {
    fn try_into_connection(self) -> Result<Connection, ConnectionParseError> {
        Ok(self)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

impl From<ConnectionParseError> for Error {
    fn from(error: ConnectionParseError) -> Error {
        Error::Parse(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(error) => std::fmt::Display::fmt(error, f),
            Error::Parse(error) => std::fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for Error {}

impl std::iter::FromIterator<Connection> for Circuit {
    fn from_iter<T: IntoIterator<Item = Connection>>(iter: T) -> Self {
        let mut circuit = Circuit::default();
        std::iter::Extend::extend(&mut circuit, iter);
        circuit
    }
}

impl std::iter::Extend<Connection> for Circuit {
    fn extend<T: IntoIterator<Item = Connection>>(&mut self, iter: T) {
        for connection in iter {
            self.connections
                .insert(connection.output.clone(), connection);
        }
    }
}

impl std::iter::FromIterator<Connection> for CachedCircuit {
    fn from_iter<T: IntoIterator<Item = Connection>>(iter: T) -> Self {
        let mut circuit = Circuit::default();
        std::iter::Extend::extend(&mut circuit, iter);
        CachedCircuit::from(circuit)
    }
}

impl std::iter::Extend<Connection> for CachedCircuit {
    fn extend<T: IntoIterator<Item = Connection>>(&mut self, iter: T) {
        for connection in iter {
            self.circuit
                .connections
                .insert(connection.output.clone(), connection);
        }
        self.cache.borrow_mut().clear();
    }
}
