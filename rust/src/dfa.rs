//! DFA construction and set operations for regex languages.
//!
//! Pipeline: regex string → regex_syntax HIR → NFA → DFA (subset construction)
//! Then: intersection (product), complement, emptiness, subset, cardinality.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use regex_syntax::hir::{Hir, HirKind, Class, Literal, Look};
use regex_syntax::Parser;

/// A byte (0..=255) plus a "wildcard" / default transition concept.
/// We use u16 so we can represent 0..=255 as symbols and use 256 for "dead".
type Symbol = u8;

const ALPHABET_SIZE: usize = 256;

// ─── NFA ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Nfa {
    /// Transition: state -> symbol -> set of target states
    transitions: Vec<HashMap<Symbol, Vec<usize>>>,
    /// Epsilon transitions: state -> set of target states
    epsilon: Vec<Vec<usize>>,
    start: usize,
    accept: usize,
    num_states: usize,
}

impl Nfa {
    fn new_state(&mut self) -> usize {
        let s = self.num_states;
        self.num_states += 1;
        self.transitions.push(HashMap::new());
        self.epsilon.push(Vec::new());
        s
    }

    fn add_transition(&mut self, from: usize, sym: Symbol, to: usize) {
        self.transitions[from].entry(sym).or_default().push(to);
    }

    fn add_epsilon(&mut self, from: usize, to: usize) {
        self.epsilon[from].push(to);
    }

    fn empty() -> Nfa {
        let mut nfa = Nfa {
            transitions: Vec::new(),
            epsilon: Vec::new(),
            start: 0,
            accept: 1,
            num_states: 0,
        };
        nfa.new_state(); // 0 = start
        nfa.new_state(); // 1 = accept
        nfa.add_epsilon(0, 1);
        nfa
    }

    /// Build NFA for a single character class range.
    fn from_char_range(ranges: &[(u8, u8)]) -> Nfa {
        let mut nfa = Nfa {
            transitions: Vec::new(),
            epsilon: Vec::new(),
            start: 0,
            accept: 1,
            num_states: 0,
        };
        nfa.new_state(); // start
        nfa.new_state(); // accept
        for &(lo, hi) in ranges {
            for c in lo..=hi {
                nfa.add_transition(0, c, 1);
            }
        }
        nfa
    }

    /// Build NFA for dot (any character except newline by default).
    fn dot() -> Nfa {
        let mut ranges = Vec::new();
        // Match any byte except \n (0x0a)
        ranges.push((0u8, 9u8));
        ranges.push((11u8, 255u8));
        Self::from_char_range(&ranges)
    }

    /// Concatenation: self followed by other.
    fn concat(mut self, other: Nfa) -> Nfa {
        let offset = self.num_states;
        // Merge other's states into self
        for _ in 0..other.num_states {
            self.new_state();
        }
        for (s, trans) in other.transitions.iter().enumerate() {
            for (&sym, targets) in trans {
                for &t in targets {
                    self.add_transition(s + offset, sym, t + offset);
                }
            }
        }
        for (s, eps) in other.epsilon.iter().enumerate() {
            for &t in eps {
                self.add_epsilon(s + offset, t + offset);
            }
        }
        // Connect self.accept → other.start via epsilon
        self.add_epsilon(self.accept, other.start + offset);
        self.accept = other.accept + offset;
        self
    }

    /// Alternation: self | other.
    fn union(mut self, other: Nfa) -> Nfa {
        let offset = self.num_states;
        for _ in 0..other.num_states {
            self.new_state();
        }
        for (s, trans) in other.transitions.iter().enumerate() {
            for (&sym, targets) in trans {
                for &t in targets {
                    self.add_transition(s + offset, sym, t + offset);
                }
            }
        }
        for (s, eps) in other.epsilon.iter().enumerate() {
            for &t in eps {
                self.add_epsilon(s + offset, t + offset);
            }
        }

        let new_start = self.new_state();
        let new_accept = self.new_state();
        self.add_epsilon(new_start, self.start);
        self.add_epsilon(new_start, other.start + offset);
        self.add_epsilon(self.accept, new_accept);
        self.add_epsilon(other.accept + offset, new_accept);
        self.start = new_start;
        self.accept = new_accept;
        self
    }

    /// Kleene star: self*.
    fn star(mut self) -> Nfa {
        let new_start = self.new_state();
        let new_accept = self.new_state();
        self.add_epsilon(new_start, self.start);
        self.add_epsilon(new_start, new_accept);
        self.add_epsilon(self.accept, self.start);
        self.add_epsilon(self.accept, new_accept);
        self.start = new_start;
        self.accept = new_accept;
        self
    }

    /// One or more: self+.
    fn plus(mut self) -> Nfa {
        let new_start = self.new_state();
        let new_accept = self.new_state();
        self.add_epsilon(new_start, self.start);
        self.add_epsilon(self.accept, self.start);
        self.add_epsilon(self.accept, new_accept);
        self.start = new_start;
        self.accept = new_accept;
        self
    }

    /// Optional: self?.
    fn optional(mut self) -> Nfa {
        let new_start = self.new_state();
        let new_accept = self.new_state();
        self.add_epsilon(new_start, self.start);
        self.add_epsilon(new_start, new_accept);
        self.add_epsilon(self.accept, new_accept);
        self.start = new_start;
        self.accept = new_accept;
        self
    }

    /// Repeat exactly n times.
    fn repeat_exact(self, n: u32) -> Nfa {
        if n == 0 {
            return Nfa::empty();
        }
        let mut result = self.clone();
        for _ in 1..n {
            result = result.concat(self.clone());
        }
        result
    }

    /// Repeat at least min, at most max times.
    fn repeat_range(self, min: u32, max: Option<u32>) -> Nfa {
        match max {
            Some(max) => {
                if min == 0 && max == 0 {
                    return Nfa::empty();
                }
                // Required part
                let mut result = if min > 0 {
                    self.clone().repeat_exact(min)
                } else {
                    Nfa::empty()
                };
                // Optional part
                for _ in min..max {
                    result = result.concat(self.clone().optional());
                }
                result
            }
            None => {
                // min..∞
                if min == 0 {
                    self.star()
                } else {
                    let required = self.clone().repeat_exact(min);
                    let kleene = self.star();
                    required.concat(kleene)
                }
            }
        }
    }

    /// Compute epsilon closure of a set of states.
    fn epsilon_closure(&self, states: &BTreeSet<usize>) -> BTreeSet<usize> {
        let mut closure = states.clone();
        let mut stack: Vec<usize> = states.iter().copied().collect();
        while let Some(s) = stack.pop() {
            for &t in &self.epsilon[s] {
                if closure.insert(t) {
                    stack.push(t);
                }
            }
        }
        closure
    }

    /// Move: from a set of states, follow symbol transitions.
    fn move_set(&self, states: &BTreeSet<usize>, sym: Symbol) -> BTreeSet<usize> {
        let mut result = BTreeSet::new();
        for &s in states {
            if let Some(targets) = self.transitions[s].get(&sym) {
                for &t in targets {
                    result.insert(t);
                }
            }
        }
        result
    }
}

// ─── DFA ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Dfa {
    /// transitions[state][symbol] = next_state (DEAD_STATE if no transition)
    /// Stored as flat vec: transitions[state * ALPHABET_SIZE + symbol]
    transitions: Vec<u32>,
    accepting: Vec<bool>,
    start: u32,
    num_states: u32,
}

const DEAD_STATE: u32 = u32::MAX;

impl Dfa {
    /// Build a DFA from a regex pattern string.
    /// The pattern is treated as fully anchored (matches the whole string).
    pub fn from_pattern(pattern: &str) -> Result<Dfa, String> {
        // Enable dot-matches-newline to match greenery behavior where . matches everything
        let hir = regex_syntax::ParserBuilder::new()
            .utf8(false)
            .build()
            .parse(&format!("(?s){}", pattern))
            .map_err(|e| format!("regex parse error: {}", e))?;
        let nfa = hir_to_nfa(&hir);
        Ok(nfa_to_dfa(&nfa))
    }

    /// Check if the DFA accepts the empty language.
    pub fn is_empty(&self) -> bool {
        if self.start == DEAD_STATE {
            return true;
        }
        // BFS from start, see if any accepting state is reachable
        let mut visited = vec![false; self.num_states as usize];
        let mut queue = VecDeque::new();
        visited[self.start as usize] = true;
        queue.push_back(self.start);
        while let Some(s) = queue.pop_front() {
            if self.accepting[s as usize] {
                return false;
            }
            let base = s as usize * ALPHABET_SIZE;
            for sym in 0..ALPHABET_SIZE {
                let next = self.transitions[base + sym];
                if next != DEAD_STATE && !visited[next as usize] {
                    visited[next as usize] = true;
                    queue.push_back(next);
                }
            }
        }
        true
    }

    /// Check if the DFA matches a string (full match, anchored).
    pub fn matches(&self, s: &str) -> bool {
        let mut state = self.start;
        if state == DEAD_STATE {
            return false;
        }
        for &b in s.as_bytes() {
            let next = self.transitions[state as usize * ALPHABET_SIZE + b as usize];
            if next == DEAD_STATE {
                return false;
            }
            state = next;
        }
        self.accepting[state as usize]
    }

    /// Complement: flip accepting states.
    pub fn complement(&self) -> Dfa {
        // We need a complete DFA (all transitions defined) — add explicit dead state
        let mut d = self.make_complete();
        for a in d.accepting.iter_mut() {
            *a = !*a;
        }
        d
    }

    /// Make DFA complete by adding an explicit dead/sink state for missing transitions.
    fn make_complete(&self) -> Dfa {
        // Check if we already need a dead state
        let has_dead = self.transitions.iter().any(|&t| t == DEAD_STATE);
        if !has_dead {
            return self.clone();
        }

        let dead = self.num_states;
        let new_num = self.num_states + 1;
        let mut transitions = Vec::with_capacity(new_num as usize * ALPHABET_SIZE);

        for s in 0..self.num_states as usize {
            let base = s * ALPHABET_SIZE;
            for sym in 0..ALPHABET_SIZE {
                let t = self.transitions[base + sym];
                transitions.push(if t == DEAD_STATE { dead } else { t });
            }
        }
        // Dead state transitions to itself
        for _ in 0..ALPHABET_SIZE {
            transitions.push(dead);
        }

        let mut accepting = self.accepting.clone();
        accepting.push(false); // dead state is not accepting

        Dfa {
            transitions,
            accepting,
            start: self.start,
            num_states: new_num,
        }
    }

    /// Product construction: intersect two DFAs.
    pub fn intersect(&self, other: &Dfa) -> Dfa {
        let a = self.make_complete();
        let b = other.make_complete();
        product(&a, &b, |a_acc, b_acc| a_acc && b_acc)
    }

    /// Minimize the DFA using Hopcroft's algorithm.
    pub fn minimize(&self) -> Dfa {
        let d = self.make_complete();
        // Remove unreachable states first
        let reachable = d.reachable_states();
        if reachable.is_empty() {
            // Empty DFA
            return Dfa {
                transitions: vec![DEAD_STATE; ALPHABET_SIZE],
                accepting: vec![false],
                start: DEAD_STATE,
                num_states: 1,
            };
        }
        hopcroft_minimize(&d, &reachable)
    }

    fn reachable_states(&self) -> Vec<u32> {
        if self.start == DEAD_STATE {
            return Vec::new();
        }
        let mut visited = vec![false; self.num_states as usize];
        let mut queue = VecDeque::new();
        visited[self.start as usize] = true;
        queue.push_back(self.start);
        while let Some(s) = queue.pop_front() {
            let base = s as usize * ALPHABET_SIZE;
            for sym in 0..ALPHABET_SIZE {
                let next = self.transitions[base + sym];
                if next != DEAD_STATE && (next as usize) < self.num_states as usize && !visited[next as usize] {
                    visited[next as usize] = true;
                    queue.push_back(next);
                }
            }
        }
        (0..self.num_states).filter(|&s| visited[s as usize]).collect()
    }

    /// Cardinality of the language. Returns None if infinite.
    pub fn cardinality(&self) -> Option<u64> {
        let minimized = self.minimize();
        // Check for cycles reachable from start that can reach an accepting state
        if minimized.has_productive_cycle() {
            return None;
        }
        // Count paths from start to accepting states (DAG)
        minimized.count_accepting_paths()
    }

    fn has_productive_cycle(&self) -> bool {
        if self.start == DEAD_STATE {
            return false;
        }
        // Find states that can reach an accepting state
        let productive = self.productive_states();
        // Check for cycles among productive states using DFS
        let n = self.num_states as usize;
        let mut color = vec![0u8; n]; // 0=white, 1=gray, 2=black
        fn dfs(s: usize, d: &Dfa, color: &mut [u8], productive: &[bool]) -> bool {
            color[s] = 1;
            let base = s * ALPHABET_SIZE;
            let mut seen = HashSet::new();
            for sym in 0..ALPHABET_SIZE {
                let next = d.transitions[base + sym];
                if next == DEAD_STATE || next as usize >= d.num_states as usize {
                    continue;
                }
                let next = next as usize;
                if !productive[next] || !seen.insert(next) {
                    continue;
                }
                if color[next] == 1 {
                    return true; // back edge = cycle
                }
                if color[next] == 0 && dfs(next, d, color, productive) {
                    return true;
                }
            }
            color[s] = 2;
            false
        }
        if productive.get(self.start as usize) == Some(&true) {
            dfs(self.start as usize, self, &mut color, &productive)
        } else {
            false
        }
    }

    fn productive_states(&self) -> Vec<bool> {
        let n = self.num_states as usize;
        let mut productive = vec![false; n];
        // Accepting states are productive
        for (i, &acc) in self.accepting.iter().enumerate() {
            if acc {
                productive[i] = true;
            }
        }
        // Fixed-point: a state is productive if it has a transition to a productive state
        loop {
            let mut changed = false;
            for s in 0..n {
                if productive[s] {
                    continue;
                }
                let base = s * ALPHABET_SIZE;
                for sym in 0..ALPHABET_SIZE {
                    let next = self.transitions[base + sym];
                    if next != DEAD_STATE && next < self.num_states && productive[next as usize] {
                        productive[s] = true;
                        changed = true;
                        break;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        productive
    }

    fn count_accepting_paths(&self) -> Option<u64> {
        if self.start == DEAD_STATE {
            return Some(0);
        }
        // Topological sort, then count paths
        let n = self.num_states as usize;
        let topo = self.topological_sort()?;
        let mut count = vec![0u64; n];
        count[self.start as usize] = 1;
        let mut total = 0u64;
        if self.accepting[self.start as usize] {
            total = 1;
        }
        for &s in &topo {
            if count[s as usize] == 0 {
                continue;
            }
            let base = s as usize * ALPHABET_SIZE;
            let mut seen = HashMap::new();
            for sym in 0..ALPHABET_SIZE {
                let next = self.transitions[base + sym];
                if next != DEAD_STATE && (next as usize) < n {
                    *seen.entry(next).or_insert(0u64) += 1;
                }
            }
            for (next, multiplicity) in seen {
                let add = count[s as usize].checked_mul(multiplicity)?;
                count[next as usize] = count[next as usize].checked_add(add)?;
                if self.accepting[next as usize] {
                    total = total.checked_add(add)?;
                }
            }
        }
        Some(total)
    }

    fn topological_sort(&self) -> Option<Vec<u32>> {
        let n = self.num_states as usize;
        let mut in_degree = vec![0u32; n];
        let mut adj: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); n];
        for s in 0..n {
            let base = s * ALPHABET_SIZE;
            for sym in 0..ALPHABET_SIZE {
                let next = self.transitions[base + sym];
                if next != DEAD_STATE && (next as usize) < n && next as usize != s {
                    if adj[s].insert(next) {
                        in_degree[next as usize] += 1;
                    }
                }
            }
        }
        let mut queue: VecDeque<u32> = VecDeque::new();
        for s in 0..n {
            if in_degree[s] == 0 {
                queue.push_back(s as u32);
            }
        }
        let mut order = Vec::with_capacity(n);
        while let Some(s) = queue.pop_front() {
            order.push(s);
            for &next in &adj[s as usize] {
                in_degree[next as usize] -= 1;
                if in_degree[next as usize] == 0 {
                    queue.push_back(next);
                }
            }
        }
        if order.len() == n {
            Some(order)
        } else {
            None // cycle
        }
    }

    /// Enumerate all strings in the language (up to a limit). Returns None if too many or infinite.
    pub fn enumerate_strings(&self, limit: usize) -> Option<Vec<String>> {
        self.cardinality()?; // Check finite
        let mut result = Vec::new();
        let mut queue: VecDeque<(u32, Vec<u8>)> = VecDeque::new();
        if self.start == DEAD_STATE {
            return Some(result);
        }
        queue.push_back((self.start, Vec::new()));
        while let Some((state, path)) = queue.pop_front() {
            if result.len() >= limit {
                return None;
            }
            if self.accepting[state as usize] {
                result.push(String::from_utf8_lossy(&path).to_string());
            }
            let base = state as usize * ALPHABET_SIZE;
            let mut seen: HashMap<u32, u8> = HashMap::new();
            for sym in 0u16..256u16 {
            let sym = sym as u8;
                let next = self.transitions[base + sym as usize];
                if next != DEAD_STATE && (next as usize) < self.num_states as usize {
                    // Only follow first symbol to each state to avoid exponential blowup
                    seen.entry(next).or_insert_with(|| {
                        let mut new_path = path.clone();
                        new_path.push(sym);
                        queue.push_back((next, new_path));
                        sym
                    });
                }
            }
        }
        Some(result)
    }

    /// Convert DFA back to regex string (simplified).
    pub fn to_regex(&self) -> String {
        dfa_to_regex(self)
    }
}

// ─── HIR → NFA ──────────────────────────────────────────────────────────

fn hir_to_nfa(hir: &Hir) -> Nfa {
    match hir.kind() {
        HirKind::Empty => Nfa::empty(),
        HirKind::Literal(Literal(bytes)) => {
            // Literal is a sequence of bytes
            let mut result = Nfa::empty();
            for &b in bytes.iter() {
                let char_nfa = Nfa::from_char_range(&[(b, b)]);
                result = result.concat(char_nfa);
            }
            result
        }
        HirKind::Class(class) => {
            match class {
                Class::Unicode(cls) => {
                    let ranges: Vec<(u8, u8)> = cls.iter()
                        .flat_map(|r| {
                            let lo = r.start() as u32;
                            let hi = r.end() as u32;
                            // Only handle ASCII/Latin-1 for now
                            let lo = lo.min(255) as u8;
                            let hi = hi.min(255) as u8;
                            if lo <= hi { Some((lo, hi)) } else { None }
                        })
                        .collect();
                    if ranges.is_empty() {
                        // No valid ranges — empty language for single char
                        let mut nfa = Nfa {
                            transitions: Vec::new(),
                            epsilon: Vec::new(),
                            start: 0,
                            accept: 1,
                            num_states: 0,
                        };
                        nfa.new_state();
                        nfa.new_state();
                        // No transitions — can never reach accept
                        nfa
                    } else {
                        Nfa::from_char_range(&ranges)
                    }
                }
                Class::Bytes(cls) => {
                    let ranges: Vec<(u8, u8)> = cls.iter()
                        .map(|r| (r.start(), r.end()))
                        .collect();
                    Nfa::from_char_range(&ranges)
                }
            }
        }
        HirKind::Look(look) => {
            // Anchors — for our fully-anchored DFA, we treat ^ and $ as empty
            match look {
                Look::Start | Look::End |
                Look::StartLF | Look::EndLF |
                Look::StartCRLF | Look::EndCRLF => Nfa::empty(),
                _ => Nfa::empty(),
            }
        }
        HirKind::Repetition(rep) => {
            let sub = hir_to_nfa(&rep.sub);
            let min = rep.min;
            let max = rep.max;
            match max {
                Some(max) => sub.repeat_range(min, Some(max)),
                None => sub.repeat_range(min, None),
            }
        }
        HirKind::Capture(cap) => {
            hir_to_nfa(&cap.sub)
        }
        HirKind::Concat(subs) => {
            let mut result = Nfa::empty();
            for sub in subs {
                result = result.concat(hir_to_nfa(sub));
            }
            result
        }
        HirKind::Alternation(subs) => {
            let mut iter = subs.iter();
            let first = iter.next().unwrap();
            let mut result = hir_to_nfa(first);
            for sub in iter {
                result = result.union(hir_to_nfa(sub));
            }
            result
        }
    }
}

// ─── NFA → DFA (subset construction) ───────────────────────────────────

fn nfa_to_dfa(nfa: &Nfa) -> Dfa {
    let start_set = {
        let mut s = BTreeSet::new();
        s.insert(nfa.start);
        nfa.epsilon_closure(&s)
    };

    let mut state_map: HashMap<Vec<usize>, u32> = HashMap::new();
    let mut dfa_transitions: Vec<u32> = Vec::new();
    let mut dfa_accepting: Vec<bool> = Vec::new();
    let mut queue: VecDeque<Vec<usize>> = VecDeque::new();

    let start_vec: Vec<usize> = start_set.iter().copied().collect();
    state_map.insert(start_vec.clone(), 0);
    dfa_transitions.resize(ALPHABET_SIZE, DEAD_STATE);
    dfa_accepting.push(start_set.contains(&nfa.accept));
    queue.push_back(start_vec);

    let mut num_states: u32 = 1;

    while let Some(current) = queue.pop_front() {
        let current_id = state_map[&current];
        let current_set: BTreeSet<usize> = current.iter().copied().collect();

        for sym in 0u16..256u16 {
            let sym = sym as u8;
            let moved = nfa.move_set(&current_set, sym);
            if moved.is_empty() {
                continue;
            }
            let closed = nfa.epsilon_closure(&moved);
            let closed_vec: Vec<usize> = closed.iter().copied().collect();

            let next_id = if let Some(&id) = state_map.get(&closed_vec) {
                id
            } else {
                let id = num_states;
                num_states += 1;
                state_map.insert(closed_vec.clone(), id);
                dfa_transitions.resize((id as usize + 1) * ALPHABET_SIZE, DEAD_STATE);
                dfa_accepting.push(closed.contains(&nfa.accept));
                queue.push_back(closed_vec);
                id
            };

            dfa_transitions[current_id as usize * ALPHABET_SIZE + sym as usize] = next_id;
        }
    }

    Dfa {
        transitions: dfa_transitions,
        accepting: dfa_accepting,
        start: 0,
        num_states,
    }
}

// ─── Product construction ───────────────────────────────────────────────

fn product<F>(a: &Dfa, b: &Dfa, accept_fn: F) -> Dfa
where
    F: Fn(bool, bool) -> bool,
{
    let mut state_map: HashMap<(u32, u32), u32> = HashMap::new();
    let mut transitions: Vec<u32> = Vec::new();
    let mut accepting: Vec<bool> = Vec::new();
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

    let start_pair = (a.start, b.start);
    state_map.insert(start_pair, 0);
    transitions.resize(ALPHABET_SIZE, DEAD_STATE);
    accepting.push(accept_fn(
        a.accepting[a.start as usize],
        b.accepting[b.start as usize],
    ));
    queue.push_back(start_pair);

    let mut num_states: u32 = 1;

    while let Some((sa, sb)) = queue.pop_front() {
        let current_id = state_map[&(sa, sb)];
        let base_a = sa as usize * ALPHABET_SIZE;
        let base_b = sb as usize * ALPHABET_SIZE;

        for sym in 0..ALPHABET_SIZE {
            let na = a.transitions[base_a + sym];
            let nb = b.transitions[base_b + sym];

            let pair = (na, nb);
            let next_id = if let Some(&id) = state_map.get(&pair) {
                id
            } else {
                let id = num_states;
                num_states += 1;
                state_map.insert(pair, id);
                transitions.resize((id as usize + 1) * ALPHABET_SIZE, DEAD_STATE);
                accepting.push(accept_fn(
                    a.accepting[na as usize],
                    b.accepting[nb as usize],
                ));
                queue.push_back(pair);
                id
            };

            transitions[current_id as usize * ALPHABET_SIZE + sym] = next_id;
        }
    }

    Dfa {
        transitions,
        accepting,
        start: 0,
        num_states,
    }
}

// ─── Hopcroft minimization ─────────────────────────────────────────────

fn hopcroft_minimize(d: &Dfa, reachable: &[u32]) -> Dfa {
    let n = d.num_states as usize;
    if reachable.is_empty() {
        return Dfa {
            transitions: vec![DEAD_STATE; ALPHABET_SIZE],
            accepting: vec![false],
            start: DEAD_STATE,
            num_states: 1,
        };
    }

    let reach_set: HashSet<u32> = reachable.iter().copied().collect();

    // Partition into accepting and non-accepting (among reachable states)
    let mut accepting_set: BTreeSet<u32> = BTreeSet::new();
    let mut non_accepting_set: BTreeSet<u32> = BTreeSet::new();
    for &s in reachable {
        if d.accepting[s as usize] {
            accepting_set.insert(s);
        } else {
            non_accepting_set.insert(s);
        }
    }

    let mut partitions: Vec<BTreeSet<u32>> = Vec::new();
    if !accepting_set.is_empty() {
        partitions.push(accepting_set.clone());
    }
    if !non_accepting_set.is_empty() {
        partitions.push(non_accepting_set);
    }
    if partitions.is_empty() {
        return Dfa {
            transitions: vec![DEAD_STATE; ALPHABET_SIZE],
            accepting: vec![false],
            start: DEAD_STATE,
            num_states: 1,
        };
    }

    // Map state -> partition index
    let mut state_to_part = vec![0usize; n];
    for (i, part) in partitions.iter().enumerate() {
        for &s in part {
            state_to_part[s as usize] = i;
        }
    }

    // Refinement loop
    let mut changed = true;
    while changed {
        changed = false;
        let mut new_partitions: Vec<BTreeSet<u32>> = Vec::new();

        for part in &partitions {
            if part.len() <= 1 {
                new_partitions.push(part.clone());
                continue;
            }

            // Split based on transitions
            let mut groups: HashMap<Vec<usize>, BTreeSet<u32>> = HashMap::new();
            for &s in part {
                let signature: Vec<usize> = (0..ALPHABET_SIZE)
                    .map(|sym| {
                        let next = d.transitions[s as usize * ALPHABET_SIZE + sym];
                        if next == DEAD_STATE || !reach_set.contains(&next) {
                            usize::MAX
                        } else {
                            state_to_part[next as usize]
                        }
                    })
                    .collect();
                groups.entry(signature).or_default().insert(s);
            }

            if groups.len() > 1 {
                changed = true;
            }
            for (_, group) in groups {
                new_partitions.push(group);
            }
        }

        partitions = new_partitions;
        for (i, part) in partitions.iter().enumerate() {
            for &s in part {
                state_to_part[s as usize] = i;
            }
        }
    }

    // Build minimized DFA
    let new_num = partitions.len() as u32;
    let mut new_transitions = vec![DEAD_STATE; partitions.len() * ALPHABET_SIZE];
    let mut new_accepting = vec![false; partitions.len()];
    let mut new_start = 0u32;

    for (i, part) in partitions.iter().enumerate() {
        let representative = *part.iter().next().unwrap();
        new_accepting[i] = d.accepting[representative as usize];
        if part.contains(&d.start) {
            new_start = i as u32;
        }
        let base = representative as usize * ALPHABET_SIZE;
        for sym in 0..ALPHABET_SIZE {
            let next = d.transitions[base + sym];
            if next != DEAD_STATE && reach_set.contains(&next) {
                new_transitions[i * ALPHABET_SIZE + sym] = state_to_part[next as usize] as u32;
            }
        }
    }

    Dfa {
        transitions: new_transitions,
        accepting: new_accepting,
        start: new_start,
        num_states: new_num,
    }
}

// ─── DFA → regex (state elimination) ───────────────────────────────────

fn dfa_to_regex(d: &Dfa) -> String {
    let d = d.minimize();
    let n = d.num_states as usize;
    if n == 0 || d.start == DEAD_STATE {
        return String::new();
    }

    // Build a GNFA (generalized NFA with regex labels on edges)
    // We add a new start state and a new accept state
    let total = n + 2;
    let new_start = n;
    let new_accept = n + 1;

    // edges[i][j] = regex string for the edge from i to j (None = no edge)
    let mut edges: Vec<Vec<Option<String>>> = vec![vec![None; total]; total];

    // Add edge from new_start to old start
    edges[new_start][d.start as usize] = Some(String::new()); // empty string = epsilon

    // Add edges from old accepting states to new_accept
    for s in 0..n {
        if d.accepting[s] {
            match &edges[s][new_accept] {
                None => edges[s][new_accept] = Some(String::new()),
                Some(_) => {} // already connected
            }
        }
    }

    // Add DFA transition edges
    for s in 0..n {
        let base = s * ALPHABET_SIZE;
        let mut targets: HashMap<u32, Vec<u8>> = HashMap::new();
        for sym in 0..ALPHABET_SIZE {
            let next = d.transitions[base + sym];
            if next != DEAD_STATE && (next as usize) < n {
                targets.entry(next).or_default().push(sym as u8);
            }
        }
        for (next, syms) in targets {
            let label = bytes_to_char_class(&syms);
            let existing = &edges[s][next as usize];
            edges[s][next as usize] = Some(match existing {
                None => label,
                Some(prev) => format!("{}|{}", prev, label),
            });
        }
    }

    // State elimination: remove states 0..n one by one
    for remove in 0..n {
        if remove == new_start || remove == new_accept {
            continue;
        }
        // Get self-loop
        let self_loop = edges[remove][remove].clone();

        for i in 0..total {
            if i == remove {
                continue;
            }
            let e_i_r = match &edges[i][remove] {
                Some(e) => e.clone(),
                None => continue,
            };

            for j in 0..total {
                if j == remove {
                    continue;
                }
                let e_r_j = match &edges[remove][j] {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // New edge from i to j: e_i_r . self_loop* . e_r_j
                let mut parts = Vec::new();
                if !e_i_r.is_empty() {
                    parts.push(wrap_if_needed(&e_i_r));
                }
                if let Some(ref sl) = self_loop {
                    if !sl.is_empty() {
                        let wrapped = wrap_if_needed(sl);
                        parts.push(format!("{}*", wrapped));
                    }
                }
                if !e_r_j.is_empty() {
                    parts.push(wrap_if_needed(&e_r_j));
                }
                let new_label = parts.join("");

                edges[i][j] = Some(match &edges[i][j] {
                    None => new_label,
                    Some(prev) if prev.is_empty() && new_label.is_empty() => String::new(),
                    Some(prev) if prev.is_empty() => format!("({})?", new_label),
                    Some(prev) if new_label.is_empty() => format!("({})?", prev),
                    Some(prev) => format!("{}|{}", prev, new_label),
                });
            }
        }

        // Remove all edges to/from removed state
        for i in 0..total {
            edges[i][remove] = None;
            edges[remove][i] = None;
        }
    }

    match &edges[new_start][new_accept] {
        Some(r) => r.clone(),
        None => String::new(),
    }
}

fn bytes_to_char_class(bytes: &[u8]) -> String {
    if bytes.len() == ALPHABET_SIZE {
        return ".".to_string();
    }
    if bytes.len() == ALPHABET_SIZE - 1 {
        // All except one character — check if it's \n (dot)
        let missing: Vec<u8> = (0..=255u8).filter(|b| !bytes.contains(b)).collect();
        if missing == vec![b'\n'] {
            return ".".to_string();
        }
    }
    if bytes.len() == 1 {
        return escape_byte(bytes[0]);
    }

    // Build ranges
    let mut sorted = bytes.to_vec();
    sorted.sort();
    sorted.dedup();

    let mut ranges: Vec<(u8, u8)> = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let start = sorted[i];
        let mut end = start;
        while i + 1 < sorted.len() && sorted[i + 1] == end + 1 {
            i += 1;
            end = sorted[i];
        }
        ranges.push((start, end));
        i += 1;
    }

    if ranges.len() == 1 && ranges[0].0 == ranges[0].1 {
        return escape_byte(ranges[0].0);
    }

    let mut result = String::from("[");
    for (lo, hi) in &ranges {
        if lo == hi {
            result.push_str(&escape_byte_in_class(*lo));
        } else if *hi == *lo + 1 {
            result.push_str(&escape_byte_in_class(*lo));
            result.push_str(&escape_byte_in_class(*hi));
        } else {
            result.push_str(&escape_byte_in_class(*lo));
            result.push('-');
            result.push_str(&escape_byte_in_class(*hi));
        }
    }
    result.push(']');
    result
}

fn escape_byte(b: u8) -> String {
    let c = b as char;
    match c {
        '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' | '^' | '$' => {
            format!("\\{}", c)
        }
        c if c.is_ascii_graphic() || c == ' ' => c.to_string(),
        _ => format!("\\x{:02x}", b),
    }
}

fn escape_byte_in_class(b: u8) -> String {
    let c = b as char;
    match c {
        ']' | '\\' | '^' | '-' => format!("\\{}", c),
        c if c.is_ascii_graphic() || c == ' ' => c.to_string(),
        _ => format!("\\x{:02x}", b),
    }
}

fn wrap_if_needed(s: &str) -> String {
    if s.len() <= 1 {
        return s.to_string();
    }
    // Check if it's already a single unit (character class, escaped char, or group)
    if s.starts_with('[') && s.ends_with(']') {
        return s.to_string();
    }
    if s.starts_with('(') && s.ends_with(')') {
        return s.to_string();
    }
    if s.starts_with('\\') && s.len() == 2 {
        return s.to_string();
    }
    if s.contains('|') || s.len() > 1 {
        return format!("({})", s);
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_match() {
        let d = Dfa::from_pattern("abc").unwrap();
        assert!(d.matches("abc"));
        assert!(!d.matches("ab"));
        assert!(!d.matches("abcd"));
    }

    #[test]
    fn test_char_class() {
        let d = Dfa::from_pattern("[a-z]+").unwrap();
        assert!(d.matches("hello"));
        assert!(!d.matches("Hello"));
        assert!(!d.matches(""));
    }

    #[test]
    fn test_intersection() {
        let a = Dfa::from_pattern("[a-z]+").unwrap();
        let b = Dfa::from_pattern("[a-z]*").unwrap();
        let inter = a.intersect(&b);
        assert!(inter.matches("hello"));
        assert!(!inter.matches(""));
    }

    #[test]
    fn test_subset() {
        let a = Dfa::from_pattern("[a-z]+").unwrap();
        let b = Dfa::from_pattern("[a-z]*").unwrap();
        // [a-z]+ ⊆ [a-z]* should be true
        let comp_b = b.complement();
        let diff = a.intersect(&comp_b);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_complement() {
        let d = Dfa::from_pattern("abc").unwrap();
        let comp = d.complement();
        assert!(!comp.matches("abc"));
        assert!(comp.matches("ab"));
        assert!(comp.matches("xyz"));
    }

    #[test]
    fn test_cardinality_finite() {
        let d = Dfa::from_pattern("[ab]").unwrap();
        assert_eq!(d.cardinality(), Some(2));
    }

    #[test]
    fn test_cardinality_infinite() {
        let d = Dfa::from_pattern("[a-z]+").unwrap();
        assert_eq!(d.cardinality(), None);
    }

    #[test]
    fn test_dot() {
        let d = Dfa::from_pattern(".{2,4}").unwrap();
        assert!(d.matches("ab"));
        assert!(d.matches("abc"));
        assert!(d.matches("abcd"));
        assert!(!d.matches("a"));
        assert!(!d.matches("abcde"));
    }

    #[test]
    fn test_alternation() {
        let d = Dfa::from_pattern("ab|cd").unwrap();
        assert!(d.matches("ab"));
        assert!(d.matches("cd"));
        assert!(!d.matches("ac"));
    }
}
