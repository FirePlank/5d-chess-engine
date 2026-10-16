use kairos::core::perft::{parse_g_line, perft};

fn positions() -> Vec<String> {
    std::fs::read_to_string("data/perft.g").unwrap().lines().filter(|l| l.starts_with("G ")).map(String::from).collect()
}

#[test]
fn startpos() {
    let s = parse_g_line(std::fs::read_to_string("data/startpos.g").unwrap().lines().next().unwrap()).unwrap();
    // Depth 3 leaves out the 1780 actions that waste a move (33186 with them): after a black
    // knight branched to T0, white actions that branch and also move a board their branch
    // leaves out of the present.
    for (d, n) in [(1, 20), (2, 440), (3, 31406)] {
        assert_eq!(perft(&s, d, 0), n, "startpos perft({})", d);
    }
}

#[test]
fn middlegames_depth2() {
    let expected: Vec<u64> =
        std::fs::read_to_string("data/perft_d2_cap40.txt").unwrap().lines().map(|l| l.trim().parse().unwrap()).collect();
    let pos = positions();
    assert_eq!(pos.len(), expected.len());
    for (k, (line, want)) in pos.iter().zip(expected).enumerate() {
        let s = parse_g_line(line).unwrap();
        assert_eq!(perft(&s, 2, 40), want, "position {}", k);
    }
}
