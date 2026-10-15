//! Undirected graph with a matching that must cover a required vertex set
//! (Edmonds' blossom search with in-place contraction).

use super::integer_set::{Index, IntegerSet};

const NIL: Index = Index::MAX;

pub struct Graph {
    n: usize,
    adj: Vec<IntegerSet>,
}

impl Graph {
    pub fn new(n: usize) -> Self {
        Graph { n, adj: vec![IntegerSet::new(); n] }
    }

    pub fn add_edge(&mut self, u: Index, v: Index) {
        debug_assert!(u != v);
        self.adj[u as usize].insert(v);
        self.adj[v as usize].insert(u);
    }

    pub fn find_matching(&self, include: &[Index]) -> Option<Vec<(Index, Index)>> {
        if include.is_empty() {
            return Some(Vec::new());
        }
        let n = self.n;
        let mut mate = vec![NIL; n];
        let mut must_include = vec![false; n];
        for &v in include {
            must_include[v as usize] = true;
        }
        let mut parent = vec![NIL; n];
        let mut base: Vec<Index> = (0..n as Index).collect();
        let mut even = vec![false; n];
        let mut in_blossom = vec![false; n];
        let mut in_anc = vec![false; n];
        let mut queue: Vec<Index> = Vec::with_capacity(n);
        let mut path: Vec<Index> = Vec::with_capacity(n);
        let mut rpath: Vec<Index> = Vec::with_capacity(n);

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
                for neighbor in self.adj[vertex as usize].iter() {
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
                        mark(vertex, neighbor, &mut parent);
                        mark(neighbor, vertex, &mut parent);
                        for v in 0..n {
                            if in_blossom[base[v] as usize] {
                                base[v] = blossom_base;
                                if !even[v] {
                                    if !must_include[v] {
                                        reconstruct(mate[v], &parent, &mate, &mut rpath, &mut path);
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
                            reconstruct(neighbor, &parent, &mate, &mut rpath, &mut path);
                            found = true;
                            break 'bfs;
                        }
                        let next = mate[neighbor as usize];
                        if !must_include[next as usize] {
                            reconstruct(neighbor, &parent, &mate, &mut rpath, &mut path);
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
                return None;
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
        let mut result = Vec::with_capacity(include.len());
        for v in 0..n {
            if mate[v] < v as Index {
                result.push((v as Index, mate[v]));
            }
        }
        Some(result)
    }
}
