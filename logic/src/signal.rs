//! Internal implemention detail to share some logic between the cached and
//! non-cached version of the logic circuit.
use crate::{And, Assignment, CachedCircuit, Circuit, Connection, LogicGate};

pub fn explore_gates<T>(circuit: &T, connection: &Connection) -> Option<u16>
where
    T: Signal,
{
    Some(match &connection.gate {
        LogicGate::Assignment(Assignment::Direct(value)) => *value,
        LogicGate::Assignment(Assignment::Gate(cable)) => circuit.signal(cable)?,
        LogicGate::And(And::True(right)) => circuit.signal(right)?,
        LogicGate::And(And::And(left, right)) => {
            let left = circuit.signal(left)?;
            let right = circuit.signal(right)?;
            left & right
        }
        LogicGate::Or(left, right) => {
            let left = circuit.signal(left)?;
            let right = circuit.signal(right)?;
            left | right
        }
        LogicGate::Lshift(origin, shift) => {
            let value = circuit.signal(origin)?;
            value << shift
        }
        LogicGate::Rshift(origin, shift) => {
            let value = circuit.signal(origin)?;
            value >> shift
        }
        LogicGate::Not(origin) => {
            let value = circuit.signal(origin)?;
            !value
        }
    })
}

pub trait Signal {
    fn signal(&self, cable: &str) -> Option<u16>;
}

impl Signal for Circuit {
    fn signal(&self, cable: &str) -> Option<u16> {
        Circuit::signal(&self, cable)
    }
}

impl Signal for CachedCircuit {
    fn signal(&self, cable: &str) -> Option<u16> {
        CachedCircuit::signal(&self, cable)
    }
}
