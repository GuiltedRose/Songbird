//! Bounded cycle-aware frontend. Queries run during preparation, never in callbacks.
use crate::language::{Event, note};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

const MAX_EVENTS: usize = 65536;
const MAX_WORK: usize = 262144;
#[derive(Clone, Debug)]
struct Node {
    id: u64,
    kind: Arc<Kind>,
}
#[derive(Clone, Debug)]
enum Kind {
    Atom(Option<String>),
    Sequence(Vec<(f64, Node)>),
    Stack(Vec<Node>),
    Speed(Box<Node>, f64),
    Choice(Vec<Node>),
    Drop(Box<Node>, f64),
}
#[derive(Clone, Debug)]
pub struct Pattern(Node);
struct Parser<'a> {
    text: &'a str,
    at: usize,
    nodes: u64,
}
impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += self.peek().unwrap().len_utf8();
        }
    }
    fn peek(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }
    fn eat(&mut self, c: char) -> bool {
        self.ws();
        if self.peek() == Some(c) {
            self.at += c.len_utf8();
            true
        } else {
            false
        }
    }
    fn node(&mut self, kind: Kind) -> Result<Node> {
        self.nodes += 1;
        ensure!(self.nodes <= 2048, "mini AST limit 2048 nodes");
        Ok(Node {
            id: self.nodes,
            kind: Arc::new(kind),
        })
    }
    fn number(&mut self) -> Result<f64> {
        self.ws();
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            self.at += 1;
        }
        let n: f64 = self.text[start..self.at]
            .parse()
            .context("expected mini number")?;
        ensure!(n.is_finite(), "nonfinite mini number");
        Ok(n)
    }
    fn integer(&mut self) -> Result<usize> {
        let n = self.number()?;
        ensure!(
            n.fract() == 0. && (0. ..=1024.).contains(&n),
            "mini integer range 0..1024"
        );
        Ok(n as usize)
    }
    // Commas have lower precedence than sequences; | selects whole sequences.
    fn expression(&mut self, depth: usize) -> Result<Node> {
        let mut branches = vec![self.choice(depth)?];
        while self.eat(',') {
            branches.push(self.choice(depth)?);
        }
        if branches.len() == 1 {
            Ok(branches.pop().unwrap())
        } else {
            self.node(Kind::Stack(branches))
        }
    }
    fn choice(&mut self, depth: usize) -> Result<Node> {
        let mut choices = vec![self.sequence(depth)?];
        while self.eat('|') {
            choices.push(self.sequence(depth)?);
        }
        if choices.len() == 1 {
            Ok(choices.pop().unwrap())
        } else {
            self.node(Kind::Choice(choices))
        }
    }
    fn sequence(&mut self, depth: usize) -> Result<Node> {
        ensure!(depth < 24, "mini nesting limit 24");
        let mut children = vec![];
        loop {
            self.ws();
            if self.peek().is_none_or(|c| ",|]}>".contains(c)) {
                break;
            }
            let mut n = self.atom(depth + 1)?;
            let mut weight = 1.;
            let mut copies = 1;
            let mut operators = 0;
            loop {
                operators += 1;
                ensure!(operators <= 24, "mini postfix nesting limit 24");
                if self.eat('*') {
                    let speed = self.number()?;
                    ensure!(
                        (0.015625..=64.).contains(&speed),
                        "mini speed range 1/64..64"
                    );
                    n = self.node(Kind::Speed(Box::new(n), speed))?;
                } else if self.eat('/') {
                    let divisor = self.number()?;
                    ensure!(
                        (0.015625..=64.).contains(&divisor),
                        "mini divisor range 1/64..64"
                    );
                    n = self.node(Kind::Speed(Box::new(n), 1. / divisor))?;
                } else if self.eat('@') {
                    weight = self.number()?;
                    ensure!(
                        (0.015625..=1024.).contains(&weight),
                        "mini weight range 1/64..1024"
                    );
                } else if self.eat('!') {
                    copies = self.integer()?;
                    ensure!(copies > 0 && copies <= 128, "mini replication range 1..128");
                } else if self.eat('?') {
                    self.ws();
                    let probability = if self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.')
                    {
                        self.number()?
                    } else {
                        0.5
                    };
                    ensure!(
                        (0. ..=1.).contains(&probability),
                        "mini removal probability range 0..1"
                    );
                    n = self.node(Kind::Drop(Box::new(n), probability))?;
                } else if self.eat('(') {
                    let pulses = self.integer()?;
                    ensure!(
                        self.eat(','),
                        "Euclidean rhythm needs pulses,steps[,rotation]"
                    );
                    let steps = self.integer()?;
                    let rotation = if self.eat(',') { self.integer()? } else { 0 };
                    ensure!(self.eat(')'), "unclosed Euclidean rhythm");
                    ensure!(
                        steps > 0 && pulses <= steps,
                        "Euclidean rhythm requires 0 <= pulses <= steps <= 1024"
                    );
                    let hits = euclidean(pulses, steps);
                    let mut rhythm = Vec::with_capacity(steps);
                    for i in 0..steps {
                        // Even spacing with an onset at step zero; positive rotation moves right.
                        let index = (i + steps - rotation % steps) % steps;
                        let hit = hits[index];
                        rhythm.push((
                            1.,
                            if hit {
                                n.clone()
                            } else {
                                self.node(Kind::Atom(None))?
                            },
                        ));
                    }
                    n = self.node(Kind::Sequence(rhythm))?;
                } else {
                    break;
                }
            }
            ensure!(
                children.len() + copies <= 1024,
                "mini sequence limit 1024 steps"
            );
            for _ in 0..copies {
                children.push((weight, n.clone()));
            }
        }
        ensure!(!children.is_empty(), "empty mini group");
        if children.len() == 1 && children[0].0 == 1. {
            Ok(children.pop().unwrap().1)
        } else {
            self.node(Kind::Sequence(children))
        }
    }
    fn atom(&mut self, depth: usize) -> Result<Node> {
        self.ws();
        let opener = self.peek().context("expected mini event")?;
        if "[<{".contains(opener) {
            self.at += 1;
            let inner = self.expression(depth)?;
            let closer = match opener {
                '[' => ']',
                '<' => '>',
                _ => '}',
            };
            ensure!(self.eat(closer), "expected closing {closer}");
            if opener == '<' {
                let steps = step_count(&inner);
                return self.node(Kind::Speed(Box::new(inner), 1. / steps));
            }
            if opener == '{' {
                let branches = match &*inner.kind {
                    Kind::Stack(v) => v.clone(),
                    _ => vec![inner],
                };
                let steps = if self.eat('%') {
                    let n = self.integer()?;
                    ensure!(n > 0, "polymeter length must be positive");
                    n as f64
                } else {
                    // The documented polymeter constructor aligns to the LCM.
                    let mut length = 1usize;
                    for branch in &branches {
                        let count = step_count(branch);
                        ensure!(
                            count.fract() == 0.,
                            "weighted polymeters require explicit %length"
                        );
                        length = length / gcd(length, count as usize) * count as usize;
                        ensure!(length <= 1024, "polymeter LCM limit 1024; provide %length");
                    }
                    length as f64
                };
                let mut aligned = vec![];
                for branch in branches {
                    let speed = steps / step_count(&branch);
                    aligned.push(self.node(Kind::Speed(Box::new(branch), speed))?);
                }
                return self.node(Kind::Stack(aligned));
            }
            return Ok(inner);
        }
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| !c.is_whitespace() && !"[]<>{},|*/!@?()%".contains(c))
        {
            self.at += self.peek().unwrap().len_utf8();
        }
        ensure!(self.at > start, "unexpected mini character {opener:?}");
        let token = &self.text[start..self.at];
        ensure!(token.len() <= 128, "mini event label limit 128 bytes");
        if token == "~" || token == "-" {
            self.node(Kind::Atom(None))
        } else {
            // Unknown identifiers are explicit local sample/drum names, checked against the track.
            if token.as_bytes()[0].is_ascii_alphabetic()
                && "abcdefgABCDEFG".contains(token.chars().next().unwrap())
                && token
                    .as_bytes()
                    .get(1)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'#' || *c == b'b')
            {
                note(token).with_context(|| format!("invalid mini pitch {token:?}"))?;
            } else {
                ensure!(
                    token
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-'),
                    "invalid mini label {token:?}"
                );
            }
            self.node(Kind::Atom(Some(token.into())))
        }
    }
}
fn euclidean(pulses: usize, steps: usize) -> Vec<bool> {
    if pulses == 0 {
        return vec![false; steps];
    }
    if pulses == steps {
        return vec![true; steps];
    }
    let mut remainders = vec![pulses];
    let mut counts = vec![];
    let mut divisor = steps - pulses;
    let mut level = 0;
    loop {
        counts.push(divisor / remainders[level]);
        remainders.push(divisor % remainders[level]);
        divisor = remainders[level];
        level += 1;
        if remainders[level] <= 1 {
            break;
        }
    }
    counts.push(divisor);
    fn build(level: i32, counts: &[usize], remainders: &[usize], out: &mut Vec<bool>) {
        if level == -1 {
            out.push(false);
        } else if level == -2 {
            out.push(true);
        } else {
            for _ in 0..counts[level as usize] {
                build(level - 1, counts, remainders, out);
            }
            if remainders[level as usize] != 0 {
                build(level - 2, counts, remainders, out);
            }
        }
    }
    let mut out = Vec::with_capacity(steps);
    build(level as i32, &counts, &remainders, &mut out);
    let first = out.iter().position(|hit| *hit).unwrap();
    out.rotate_left(first);
    out
}
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
fn step_count(n: &Node) -> f64 {
    match &*n.kind {
        Kind::Sequence(v) => v.iter().map(|(w, _)| w).sum(),
        _ => 1.,
    }
}
#[derive(Clone)]
struct Hit {
    start: f64,
    end: f64,
    value: String,
}
struct Query {
    seed: u64,
    work: usize,
}
fn hash(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
fn random(seed: u64, id: u64, time: f64) -> f64 {
    let coordinate = (time * 4294967296.).round() as i64 as u64;
    (hash(seed ^ hash(id) ^ hash(coordinate)) >> 11) as f64 / (1u64 << 53) as f64
}
impl Node {
    fn query(&self, lo: f64, hi: f64, q: &mut Query) -> Result<Vec<Hit>> {
        q.work += 1;
        ensure!(
            q.work <= MAX_WORK
                && lo.is_finite()
                && hi.is_finite()
                && hi > lo
                && hi.abs() <= 1e9
                && hi - lo <= 65536.,
            "mini evaluation work limit; reduce speed, cycles or nesting"
        );
        let mut out = vec![];
        match &*self.kind {
            Kind::Atom(value) => {
                if let Some(value) = value {
                    for cycle in lo.floor() as i64..hi.ceil() as i64 {
                        out.push(Hit {
                            start: cycle as f64,
                            end: cycle as f64 + 1.,
                            value: value.clone(),
                        });
                        q.work += 1;
                        ensure!(
                            q.work <= MAX_WORK && out.len() <= MAX_EVENTS,
                            "mini event/work limit"
                        );
                    }
                }
            }
            Kind::Speed(n, speed) => {
                for mut h in n.query(lo * speed, hi * speed, q)? {
                    h.start /= speed;
                    h.end /= speed;
                    out.push(h);
                }
            }
            Kind::Stack(nodes) => {
                for n in nodes {
                    out.extend(n.query(lo, hi, q)?);
                    ensure!(out.len() <= MAX_EVENTS, "mini event limit 65536");
                }
            }
            Kind::Choice(nodes) => {
                for cycle in lo.floor() as i64..hi.ceil() as i64 {
                    let index =
                        (random(q.seed, self.id, cycle as f64) * nodes.len() as f64) as usize;
                    out.extend(nodes[index].query(
                        lo.max(cycle as f64),
                        hi.min(cycle as f64 + 1.),
                        q,
                    )?);
                    ensure!(out.len() <= MAX_EVENTS, "mini event limit 65536");
                }
            }
            Kind::Drop(n, probability) => {
                out = n.query(lo, hi, q)?;
                out.retain(|h| random(q.seed, self.id, h.start) >= *probability);
            }
            Kind::Sequence(children) => {
                let total: f64 = children.iter().map(|(w, _)| *w).sum();
                for cycle in lo.floor() as i64..hi.ceil() as i64 {
                    let base = cycle as f64;
                    let mut offset = 0.;
                    for (index, (weight, child)) in children.iter().enumerate() {
                        let width = weight / total;
                        let a = (base + offset).max(lo);
                        let b = (base + offset + width).min(hi);
                        if b - a > 1e-12 {
                            let child_lo = base + (a - base - offset) / width;
                            let child_hi = base + (b - base - offset) / width;
                            let outer_seed = q.seed;
                            q.seed = hash(outer_seed ^ hash(self.id) ^ hash(index as u64));
                            let hits = child.query(child_lo, child_hi, q)?;
                            q.seed = outer_seed;
                            for h in hits {
                                out.push(Hit {
                                    start: base + offset + (h.start.max(base) - base) * width,
                                    end: base + offset + (h.end.min(base + 1.) - base) * width,
                                    value: h.value,
                                });
                            }
                        }
                        offset += width;
                        ensure!(out.len() <= MAX_EVENTS, "mini event limit 65536");
                        q.work += 1;
                        ensure!(q.work <= MAX_WORK, "mini work limit");
                    }
                }
            }
        }
        Ok(out)
    }
}
impl Pattern {
    pub fn compile(text: &str) -> Result<Self> {
        ensure!(
            !text.trim().is_empty() && text.len() <= 65536,
            "mini expression requires 1..65536 bytes"
        );
        let mut parser = Parser {
            text,
            at: 0,
            nodes: 0,
        };
        let result = parser.expression(0);
        let node = result.with_context(|| format!("mini notation at byte {}", parser.at))?;
        parser.ws();
        ensure!(
            parser.at == text.len(),
            "mini unexpected token at byte {}",
            parser.at
        );
        Ok(Self(node))
    }
    /// Query onsets in a half-open cycle range. Durations can extend past its end.
    pub fn events(&self, start: f64, end: f64, cycle_beats: f64, seed: u64) -> Result<Vec<Event>> {
        ensure!(
            start.is_finite() && end.is_finite() && start >= 0. && end > start && end <= 65536.,
            "mini query requires 0 <= start < end <= 65536 cycles"
        );
        ensure!(
            cycle_beats.is_finite() && (0.0625..=256.).contains(&cycle_beats),
            "cycle duration range 1/16..256 quarter-note beats"
        );
        let hits = self.0.query(start, end, &mut Query { seed, work: 0 })?;
        let mut out = vec![];
        for h in hits {
            if h.start + 1e-10 < start || h.start >= end - 1e-10 || h.end <= h.start {
                continue;
            }
            let (notes, drum) = match note(&h.value) {
                Ok(n) => (vec![n], None),
                Err(_) => (vec![], Some(h.value)),
            };
            out.push(Event {
                beat: h.start * cycle_beats,
                duration: (h.end - h.start) * cycle_beats,
                notes,
                drum,
            });
        }
        out.sort_by(|a, b| {
            a.beat
                .total_cmp(&b.beat)
                .then(a.notes.cmp(&b.notes))
                .then(a.drum.cmp(&b.drum))
        });
        ensure!(out.len() <= MAX_EVENTS, "mini event limit 65536");
        Ok(out)
    }
}
