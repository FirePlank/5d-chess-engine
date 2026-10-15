//! Undirected graph with a matching that must cover a required vertex set
//! (Edmonds' blossom search with in-place contraction). Buffers are kept between uses.

use super::integer_set::{Bits, Index};

const NIL: Index = Index::MAX;

#[derive(Default)]
pub struct Graph {
    n: usize,
    /// Adjacency bitsets, `stride` words per vertex.
    stride: usize,
    adj: Vec<u64>,
    work: Work,
    result: Vec<(Index, Index)>,
}

#[derive(Default)]
struct Work {
    mate: Vec<Index>,
    must_include: Vec<bool>,
    parent: Vec<Index>,
    base: Vec<Index>,
    even: Vec<bool>,
    in_blossom: Vec<bool>,
    in_anc: Vec<bool>,
    queue: Vec<Index>,
    path: Vec<Index>,
    rpath: Vec<Index>,
}

impl Graph {
    pub fn new(n: usize) -> Self {
        let mut g = Graph::default();
        g.reset(n);
        g
    }

    /// Clear to `n` isolated vertices.
    pub fn reset(&mut self, n: usize) {
        self.n = n;
        self.stride = n.div_ceil(64).max(1);
        self.adj.clear();
        self.adj.resize(n * self.stride, 0);
    }

    pub fn add_edge(&mut self, u: Index, v: Index) {
        debug_assert!(u != v);
        let st = self.stride;
        self.adj[u as usize * st + (v as usize >> 6)] |= 1u64 << (v & 63);
        self.adj[v as usize * st + (u as usize >> 6)] |= 1u64 << (u & 63);
    }

    /// A matching covering every vertex of `include`, as pairs (u, v) with u > v in
    /// ascending order of u.
    pub fn find_matching(&mut self, include: &[Index]) -> Option<&[(Index, Index)]> {
        self.result.clear();
        if include.is_empty() {
            return Some(&self.result);
        }
        let n = self.n;
        let w = &mut self.work;
        w.mate.clear();
        w.mate.resize(n, NIL);
        w.must_include.clear();
        w.must_include.resize(n, false);
        for &v in include {
            w.must_include[v as usize] = true;
        }
        w.parent.clear();
        w.parent.resize(n, NIL);
        w.base.clear();
        w.base.extend(0..n as Index);
        w.even.clear();
        w.even.resize(n, false);
        w.in_blossom.clear();
        w.in_blossom.resize(n, false);
        w.in_anc.clear();
        w.in_anc.resize(n, false);
        if !augment_all(n, self.stride, &self.adj, include, w) {
            return None;
        }
        for v in 0..n {
            if w.mate[v] < v as Index {
                self.result.push((v as Index, w.mate[v]));
            }
        }
        Some(&self.result)
    }
}

fn augment_all(n: usize, stride: usize, adj: &[u64], include: &[Index], w: &mut Work) -> bool {
    let Work { mate, must_include, parent, base, even, in_blossom, in_anc, queue, path, rpath } = w;
    let neighbors = |v: usize| Bits::from_words(&adj[v * stride..(v + 1) * stride]);
    for &root in include {
        if mate[root as usize] != NIL {
            continue;
        }
        // ---- find_augmenting_path(root)
        parent.iter_mut().for_each(|x| *x = NIL);
        for (i, b) in base.iter_mut().enumerate() {
            *b = i as Index;
        }
        even.iter_mut().for_each(|x| *x = false);
        queue.clear();
        path.clear();

        let reconstruct = |endpoint: Index, parent: &[Index], mate: &[Index], rpath: &mut Vec<Index>, path: &mut Vec<Index>| {
            rpath.clear();
            let mut v = endpoint;
            while v != NIL {
                rpath.push(v);
                let prev = parent[v as usize];
                if prev == NIL {
                    break;
                }
                rpath.push(prev);
                v = mate[prev as usize];
            }
            path.clear();
            path.extend(rpath.iter().rev());
        };

        queue.push(root);
        even[root as usize] = true;
        let mut qpos = 0;
        let mut found = false;
        'bfs: while qpos < queue.len() {
            let vertex = queue[qpos];
            qpos += 1;
            for neighbor in neighbors(vertex as usize).iter() {
                if mate[vertex as usize] == neighbor {
                    continue;
                }
                if base[vertex as usize] == base[neighbor as usize] {
                    continue;
                }
                let closes = neighbor == root
                    || (mate[neighbor as usize] != NIL && parent[mate[neighbor as usize] as usize] != NIL);
                if closes {
                    // lowest common ancestor
                    in_anc.iter_mut().for_each(|x| *x = false);
                    let mut a = vertex;
                    loop {
                        a = base[a as usize];
                        in_anc[a as usize] = true;
                        if mate[a as usize] == NIL {
                            break;
                        }
                        a = parent[mate[a as usize] as usize];
                    }
                    let mut b = neighbor;
                    let blossom_base = loop {
                        b = base[b as usize];
                        if in_anc[b as usize] {
                            break b;
                        }
                        b = parent[mate[b as usize] as usize];
                    };
                    in_blossom.iter_mut().for_each(|x| *x = false);
                    let mut mark = |mut vtx: Index, mut child: Index, parent: &mut [Index]| {
                        while base[vtx as usize] != blossom_base {
                            in_blossom[base[vtx as usize] as usize] = true;
                            in_blossom[base[mate[vtx as usize] as usize] as usize] = true;
                            parent[vtx as usize] = child;
                            child = mate[vtx as usize];
                            vtx = parent[mate[vtx as usize] as usize];
                        }
                    };
                    mark(vertex, neighbor, parent);
                    mark(neighbor, vertex, parent);
                    for v in 0..n {
                        if in_blossom[base[v] as usize] {
                            base[v] = blossom_base;
                            if !even[v] {
                                if !must_include[v] {
                                    reconstruct(mate[v], parent, mate, rpath, path);
                                    path.push(v as Index);
                                    found = true;
                                    break 'bfs;
                                }
                                even[v] = true;
                                queue.push(v as Index);
                            }
                        }
                    }
                } else if parent[neighbor as usize] == NIL {
                    parent[neighbor as usize] = vertex;
                    if mate[neighbor as usize] == NIL {
                        reconstruct(neighbor, parent, mate, rpath, path);
                        found = true;
                        break 'bfs;
                    }
                    let next = mate[neighbor as usize];
                    if !must_include[next as usize] {
                        reconstruct(neighbor, parent, mate, rpath, path);
                        path.push(next);
                        found = true;
                        break 'bfs;
                    }
                    even[next as usize] = true;
                    queue.push(next);
                }
            }
        }
        if !found {
            return false;
        }
        let mut i = 1;
        while i + 1 < path.len() {
            mate[path[i] as usize] = NIL;
            mate[path[i + 1] as usize] = NIL;
            i += 2;
        }
        let mut i = 0;
        while i + 1 < path.len() {
            mate[path[i] as usize] = path[i + 1];
            mate[path[i + 1] as usize] = path[i];
            i += 2;
        }
    }
    true
}
