"""Instrument a fresh cf055d3 archive for this audit. Never run on the live checkout.

Usage: python docs/performance_audit/instrument.py target/perf-audit/instrumented
The argument must contain an extracted git archive, without a .git entry.
"""
from pathlib import Path
import sys
root = Path(sys.argv[1]).resolve()
assert root.name == 'instrumented' and not (root / '.git').exists(), root
def edit(path, old, new):
    p = root / path
    s = p.read_text()
    assert old in s, (path, old)
    p.write_text(s.replace(old, new))

edit('src/misc/prof.rs', '26]', '32]')
edit('src/misc/prof.rs', '"nnue_ft", "nnue_l2",', '"nnue_ft", "nnue_l2", "hce_sides", "board_chess", "local_gain", "royal_5d", "past_line", "heap_bound",')
edit('src/engine/hce.rs', 'pub fn sides_with(s: &State, src: &mut impl TermSource) -> Sides {', 'pub fn sides_with(s: &State, src: &mut impl TermSource) -> Sides {\n    let _audit = crate::misc::prof::scope(26);')
edit('src/engine/hce.rs', 'fn royal_5d(s: &State, sd: &mut Sides, src: &mut impl TermSource, atts: &[[Bitboard; 4]; ATT_LINES]) {', 'fn royal_5d(s: &State, sd: &mut Sides, src: &mut impl TermSource, atts: &[[Bitboard; 4]; ATT_LINES]) {\n    let _audit = crate::misc::prof::scope(29);')
edit('src/engine/hce.rs', 'fn past_line(g: &Geo, l: i32, c: bool) -> [i32; 2] {', 'fn past_line(g: &Geo, l: i32, c: bool) -> [i32; 2] {\n    let _audit = crate::misc::prof::scope(30);')
edit('src/engine/chess.rs', 'pub fn board_chess(b: &Board, w: i32, h: i32) -> BoardChess {', 'pub fn board_chess(b: &Board, w: i32, h: i32) -> BoardChess {\n    let _audit = crate::misc::prof::scope(27);')
edit('src/engine/local.rs', 'pub fn capture_gain(b: &Board, c: bool, height: i32, vals: &Values) -> i32 {', 'pub fn capture_gain(b: &Board, c: bool, height: i32, vals: &Values) -> i32 {\n    let _audit = crate::misc::prof::scope(28);')
# Give sequential HC build sections explicit end points. Shadowing a guard does not end it.
p = root/'src/core/hypercuboid.rs'
s = p.read_text()
for n in [15, 1, 18, 19, 12, 13, 14]:
    s = s.replace(f'let _prof = crate::misc::prof::scope({n});', f'let audit_{n} = crate::misc::prof::scope({n});')
s = s.replace('drop(());\n        let audit_1 =', 'drop(audit_15);\n        let audit_1 =')
s = s.replace('drop(());\n            let audit_19', 'drop(audit_18);\n            let audit_19')
s = s.replace('let audit_19 =', 'let _audit_19 =')
s = s.replace('drop(());\n        let max_branch', 'drop(audit_1);\n        let max_branch')
s = s.replace('drop(());\n        let audit_13', 'drop(audit_12);\n        let audit_13')
s = s.replace('drop(());\n        let audit_14', 'drop(audit_13);\n        let audit_14')
s = s.replace('let audit_14 =', 'let _audit_14 =')
s = s.replace('fn push(&mut self, hc: Hc, class: i32, fill: &mut BranchFill) {\n        let _prof = crate::misc::prof::scope(11);', 'fn push(&mut self, hc: Hc, class: i32, fill: &mut BranchFill) {\n        let _prof = crate::misc::prof::scope(31);')
p.write_text(s)

# Exact cache-hit opportunity counters; observe requests, do not reuse results.
cache = '''
thread_local! {
    static AUDIT_CACHE: std::cell::RefCell<Vec<Option<([u64; 12], i32, i32)>>> = std::cell::RefCell::new(vec![None; 4096]);
}
fn audit_request(b: &Board, a: i32, c: i32, hit: usize, miss: usize) {
    let raw = b.raw_bbs();
    let mut k = (a as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ c as u64;
    for x in raw { k = (k ^ x).wrapping_mul(0xbf58476d1ce4e5b9); k ^= k >> 29; }
    AUDIT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let slot = &mut cache[(k >> 52) as usize];
        let key = (raw, a, c);
        crate::misc::prof::count(if slot.as_ref() == Some(&key) {hit} else {miss});
        *slot = Some(key);
    });
}
'''
for name, marker, call in [
    ('chess', '    let _audit = crate::misc::prof::scope(27);', '    audit_request(b, w, h, 0, 1);'),
    ('local', '    let _audit = crate::misc::prof::scope(28);', '    audit_request(b, c as i32, height, 2, 3);')]:
    p = root/f'src/engine/{name}.rs'
    s = p.read_text().replace(marker, call+'\n'+marker)
    p.write_text(s+'\n'+cache)

# Strengthen measurement for this audit: actual visited nodes, scores, proof status,
# and every completed-depth report with clock/NPS fields removed.
def bench_trace(r):
    p = r/'src/bin/ktools.rs'
    s = p.read_text()
    old = 'let best = e.find_best_move(s, &limits, &stop, &mut |m| last = m);'
    new = '''let mut trace = Vec::new();
        let best = e.find_best_move(s, &limits, &stop, &mut |m| {
            let words: Vec<_> = m.split_whitespace().collect();
            let mut clean = Vec::new();
            let mut j = 0;
            while j < words.len() {
                if words[j] == "nps" || words[j] == "time" { j += 2; } else { clean.push(words[j]); j += 1; }
            }
            if m.starts_with("depth ") { trace.push(clean.join(" ")); }
            last = m;
        });
        trace.hash(&mut h);
        e.last_score().hash(&mut h);
        e.last_proven().hash(&mut h);'''
    assert old in s
    s = s.replace(old, new)
    old = 'let nodes: u64 = last.split_whitespace().skip_while(|w| *w != "nodes").nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);'
    assert old in s
    s = s.replace(old, 'let nodes = e.last_search_nodes();')
    p.write_text(s)
bench_trace(root)
