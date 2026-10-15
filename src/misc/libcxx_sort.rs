//! libc++-compatible introsort, so the order of equal elements is reproducible across
//! builds. Only used where tie order is observable.

pub fn sort_by<T: Copy>(v: &mut [T], mut less: impl FnMut(&T, &T) -> bool) {
    let n = v.len();
    if n < 2 {
        return;
    }
    let depth = 2 * (usize::BITS - 1 - n.leading_zeros()) as isize;
    introsort(v, 0, n, &mut less, depth, true);
}

fn sort3<T: Copy>(v: &mut [T], x: usize, y: usize, z: usize, c: &mut impl FnMut(&T, &T) -> bool) {
    if !c(&v[y], &v[x]) {
        if !c(&v[z], &v[y]) {
            return;
        }
        v.swap(y, z);
        if c(&v[y], &v[x]) {
            v.swap(x, y);
        }
        return;
    }
    if c(&v[z], &v[y]) {
        v.swap(x, z);
        return;
    }
    v.swap(x, y);
    if c(&v[z], &v[y]) {
        v.swap(y, z);
    }
}

fn sort4<T: Copy>(v: &mut [T], a: usize, b: usize, cc: usize, d: usize, c: &mut impl FnMut(&T, &T) -> bool) {
    sort3(v, a, b, cc, c);
    if c(&v[d], &v[cc]) {
        v.swap(cc, d);
        if c(&v[cc], &v[b]) {
            v.swap(b, cc);
            if c(&v[b], &v[a]) {
                v.swap(a, b);
            }
        }
    }
}

fn sort5<T: Copy>(v: &mut [T], a: usize, b: usize, cc: usize, d: usize, e: usize, c: &mut impl FnMut(&T, &T) -> bool) {
    sort4(v, a, b, cc, d, c);
    if c(&v[e], &v[d]) {
        v.swap(d, e);
        if c(&v[d], &v[cc]) {
            v.swap(cc, d);
            if c(&v[cc], &v[b]) {
                v.swap(b, cc);
                if c(&v[b], &v[a]) {
                    v.swap(a, b);
                }
            }
        }
    }
}

fn insertion_sort<T: Copy>(v: &mut [T], first: usize, last: usize, c: &mut impl FnMut(&T, &T) -> bool) {
    if first == last {
        return;
    }
    let mut i = first + 1;
    while i != last {
        let mut j = i - 1;
        if c(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !c(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
        }
        i += 1;
    }
}

fn insertion_sort_incomplete<T: Copy>(v: &mut [T], first: usize, last: usize, c: &mut impl FnMut(&T, &T) -> bool) -> bool {
    match last - first {
        0 | 1 => return true,
        2 => {
            if c(&v[last - 1], &v[first]) {
                v.swap(first, last - 1);
            }
            return true;
        }
        3 => {
            sort3(v, first, first + 1, last - 1, c);
            return true;
        }
        4 => {
            sort4(v, first, first + 1, first + 2, last - 1, c);
            return true;
        }
        5 => {
            sort5(v, first, first + 1, first + 2, first + 3, last - 1, c);
            return true;
        }
        _ => {}
    }
    let mut j = first + 2;
    sort3(v, first, first + 1, j, c);
    const LIMIT: u32 = 8;
    let mut count = 0;
    let mut i = j + 1;
    while i != last {
        if c(&v[i], &v[j]) {
            let t = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !c(&t, &v[k]) {
                    break;
                }
            }
            v[j] = t;
            count += 1;
            if count == LIMIT {
                return i + 1 == last;
            }
        }
        j = i;
        i += 1;
    }
    true
}

fn partition_with_equals_on_right<T: Copy>(
    v: &mut [T],
    begin: usize,
    end: usize,
    c: &mut impl FnMut(&T, &T) -> bool,
) -> (usize, bool) {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    loop {
        first += 1;
        if !c(&v[first], &pivot) {
            break;
        }
    }
    if begin == first - 1 {
        while first < last {
            last -= 1;
            if c(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if c(&v[last], &pivot) {
                break;
            }
        }
    }
    let already = first >= last;
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if !c(&v[first], &pivot) {
                break;
            }
        }
        loop {
            last -= 1;
            if c(&v[last], &pivot) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    if begin != pivot_pos {
        v[begin] = v[pivot_pos];
    }
    v[pivot_pos] = pivot;
    (pivot_pos, already)
}

fn partition_with_equals_on_left<T: Copy>(v: &mut [T], begin: usize, end: usize, c: &mut impl FnMut(&T, &T) -> bool) -> usize {
    let pivot = v[begin];
    let mut first = begin;
    let mut last = end;
    if c(&pivot, &v[last - 1]) {
        loop {
            first += 1;
            if c(&pivot, &v[first]) {
                break;
            }
        }
    } else {
        loop {
            first += 1;
            if !(first < last && !c(&pivot, &v[first])) {
                break;
            }
        }
    }
    if first < last {
        loop {
            last -= 1;
            if !c(&pivot, &v[last]) {
                break;
            }
        }
    }
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if c(&pivot, &v[first]) {
                break;
            }
        }
        loop {
            last -= 1;
            if !c(&pivot, &v[last]) {
                break;
            }
        }
    }
    let pivot_pos = first - 1;
    if begin != pivot_pos {
        v[begin] = v[pivot_pos];
    }
    v[pivot_pos] = pivot;
    first
}

fn heap_sort<T: Copy>(v: &mut [T], first: usize, last: usize, c: &mut impl FnMut(&T, &T) -> bool) {
    // Introsort's depth fallback; practically unreachable at the sizes used here.
    let s = &mut v[first..last];
    let n = s.len();
    let sift = |s: &mut [T], mut root: usize, n: usize, c: &mut dyn FnMut(&T, &T) -> bool| loop {
        let mut child = 2 * root + 1;
        if child >= n {
            break;
        }
        if child + 1 < n && c(&s[child], &s[child + 1]) {
            child += 1;
        }
        if !c(&s[root], &s[child]) {
            break;
        }
        s.swap(root, child);
        root = child;
    };
    for i in (0..n / 2).rev() {
        sift(s, i, n, c);
    }
    for end in (1..n).rev() {
        s.swap(0, end);
        sift(s, 0, end, c);
    }
}

fn introsort<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    c: &mut impl FnMut(&T, &T) -> bool,
    mut depth: isize,
    mut leftmost: bool,
) {
    const LIMIT: usize = 24;
    const NINTHER: usize = 128;
    loop {
        let len = last - first;
        match len {
            0 | 1 => return,
            2 => {
                if c(&v[last - 1], &v[first]) {
                    v.swap(first, last - 1);
                }
                return;
            }
            3 => {
                sort3(v, first, first + 1, last - 1, c);
                return;
            }
            4 => {
                sort4(v, first, first + 1, first + 2, last - 1, c);
                return;
            }
            5 => {
                sort5(v, first, first + 1, first + 2, first + 3, last - 1, c);
                return;
            }
            _ => {}
        }
        if len < LIMIT {
            insertion_sort(v, first, last, c);
            return;
        }
        if depth == 0 {
            heap_sort(v, first, last, c);
            return;
        }
        depth -= 1;
        let half = len / 2;
        if len > NINTHER {
            sort3(v, first, first + half, last - 1, c);
            sort3(v, first + 1, first + half - 1, last - 2, c);
            sort3(v, first + 2, first + half + 1, last - 3, c);
            sort3(v, first + half - 1, first + half, first + half + 1, c);
            v.swap(first, first + half);
        } else {
            sort3(v, first + half, first, last - 1, c);
        }
        if !leftmost && !c(&v[first - 1], &v[first]) {
            first = partition_with_equals_on_left(v, first, last, c);
            continue;
        }
        let (i, partitioned) = partition_with_equals_on_right(v, first, last, c);
        if partitioned {
            let fs = insertion_sort_incomplete(v, first, i, c);
            if insertion_sort_incomplete(v, i + 1, last, c) {
                if fs {
                    return;
                }
                last = i;
                continue;
            } else if fs {
                first = i + 1;
                continue;
            }
        }
        introsort(v, first, i, c, depth, leftmost);
        leftmost = false;
        first = i + 1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sorts() {
        let mut s = 12345u64;
        for n in 0..300 {
            let mut v: Vec<u32> = (0..n)
                .map(|_| {
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((s >> 33) % 7) as u32
                })
                .collect();
            let mut w = v.clone();
            super::sort_by(&mut v, |a, b| a < b);
            w.sort();
            assert_eq!(v, w);
        }
    }
}
