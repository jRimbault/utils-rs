use logic::Circuit;

#[test]
fn example_1() {
    let circuit = "123 -> x
456 -> y
x AND y -> d
x OR y -> e
x LSHIFT 2 -> f
y RSHIFT 2 -> g
NOT x -> h
NOT y -> i";
    let circuit = Circuit::from_string(circuit).unwrap();
    let expected_results = [
        ("error", None),
        ("d", Some(72)),
        ("e", Some(507)),
        ("f", Some(492)),
        ("g", Some(114)),
        ("h", Some(65412)),
        ("i", Some(65079)),
        ("x", Some(123)),
        ("y", Some(456)),
    ];
    for (cable, expected_signal) in expected_results {
        let signal = circuit.signal(cable);
        eprintln!("{cable}: {signal:?}");
        assert_eq!(expected_signal, signal);
    }
    let mut circuit = circuit;
    circuit.add_connection("NOT g -> error").unwrap();
    assert_eq!(circuit.signal("error"), Some(65421));
}
