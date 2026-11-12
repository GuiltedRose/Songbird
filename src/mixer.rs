//! Prepared, bounded audio routing. All names/dependencies resolve before playback.
use crate::{
    dsp::{self, Processor},
    language::{Score, Track},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Routing {
    pub sidechain: Option<String>,
    pub route: Option<String>,
    pub sends: Vec<Send>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Send {
    pub destination: String,
    pub gain: f32,
    pub pre: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub sources: Vec<String>,
    pub bus: bool,
    pub controls: Track,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Master,
    Node(usize),
}
#[derive(Clone)]
struct Edge {
    destination: Destination,
    gain: f32,
    pre: bool,
}
#[derive(Clone, Default)]
pub struct Meter {
    pub reduction_db: f32,
    pub peak: f32,
    pub clipped_frames: u64,
}
#[derive(Clone, Copy)]
enum Detector {
    Track(usize),
    Node(usize),
}
#[derive(Clone, Copy)]
enum Target {
    Gain,
    Pan,
    Filter,
    Effect(usize, usize),
}
#[derive(Clone)]
struct PreparedAutomation {
    target: Target,
    points: Vec<[f64; 2]>,
    step: bool,
}
#[derive(Clone)]
struct PreparedNode {
    controls: Track,
    detector: Option<Detector>,
    automation: Vec<PreparedAutomation>,
    effects: Vec<Processor>,
    edges: Vec<Edge>,
}
#[derive(Clone)]
pub struct Mixer {
    nodes: Vec<PreparedNode>,
    order: Vec<usize>,
    inputs: Vec<[f32; 2]>,
    tracks: Vec<Vec<Edge>>,
    track_outputs: Vec<[f32; 2]>,
    outputs: Vec<[f32; 2]>,
    audible: Vec<bool>,
    pub names: Vec<String>,
    pub meters: Vec<Meter>,
}
fn destination(name: &str, names: &BTreeMap<String, usize>) -> Result<Destination> {
    if name == "master" {
        Ok(Destination::Master)
    } else {
        names
            .get(name)
            .copied()
            .map(Destination::Node)
            .with_context(|| format!("unknown mixer destination {name:?}"))
    }
}
fn edges(
    routing: &Routing,
    default: Option<usize>,
    names: &BTreeMap<String, usize>,
) -> Result<Vec<Edge>> {
    ensure!(routing.sends.len() <= 32, "maximum 32 sends per channel");
    let direct = if let Some(name) = &routing.route {
        destination(name, names)?
    } else {
        default.map_or(Destination::Master, Destination::Node)
    };
    if let Some(owner) = default {
        ensure!(
            direct == Destination::Node(owner),
            "track route conflicts with explicit source channel"
        );
    }
    let mut result = vec![Edge {
        destination: direct,
        gain: 1.,
        pre: false,
    }];
    for send in &routing.sends {
        ensure!(
            send.gain.is_finite() && (0. ..=4.).contains(&send.gain),
            "send gain range 0..4"
        );
        result.push(Edge {
            destination: destination(&send.destination, names)?,
            gain: send.gain,
            pre: send.pre,
        });
    }
    Ok(result)
}
fn emit(
    edges: &[Edge],
    pre: [f32; 2],
    post: [f32; 2],
    inputs: &mut [[f32; 2]],
    master: &mut [f32; 2],
) {
    for edge in edges {
        let frame = if edge.pre { pre } else { post };
        let output = match edge.destination {
            Destination::Master => &mut *master,
            Destination::Node(i) => &mut inputs[i],
        };
        for ch in 0..2 {
            output[ch] += frame[ch] * edge.gain;
        }
    }
}
/// Mixer balance is unity at center; legacy track equal-power panning is unchanged.
fn balance(x: [f32; 2], pan: f32) -> [f32; 2] {
    [x[0] * (1. - pan.max(0.)), x[1] * (1. + pan.min(0.))]
}
impl Mixer {
    pub fn compile(score: &Score) -> Result<Option<Self>> {
        let active = !score.mixer.is_empty()
            || score.tracks.iter().any(|t| {
                t.routing.route.is_some()
                    || !t.routing.sends.is_empty()
                    || t.routing.sidechain.is_some()
            });
        if !active {
            return Ok(None);
        }
        ensure!(
            score.tracks.len() <= 128 && score.mixer.len() <= 64,
            "mixer limits: 128 tracks, 64 explicit channels/buses"
        );
        let mut names = BTreeMap::new();
        let mut track_names = BTreeMap::new();
        for (i, track) in score.tracks.iter().enumerate() {
            ensure!(
                track.name != "master" && track_names.insert(track.name.clone(), i).is_none(),
                "duplicate/reserved source name {:?}",
                track.name
            );
        }
        for (i, node) in score.mixer.iter().enumerate() {
            let name = &node.controls.name;
            ensure!(
                name != "master"
                    && !track_names.contains_key(name)
                    && names.insert(name.clone(), i).is_none(),
                "duplicate/reserved mixer node {name:?}"
            );
        }
        let mut owners = vec![None; score.tracks.len()];
        for (i, node) in score.mixer.iter().enumerate() {
            ensure!(
                !node.bus || node.sources.is_empty(),
                "bus sources arrive through route/send; source is for explicit channels"
            );
            ensure!(
                node.bus || !node.sources.is_empty(),
                "explicit channel {:?} needs at least one source",
                node.controls.name
            );
            for source in &node.sources {
                let index = *track_names
                    .get(source)
                    .with_context(|| format!("unknown channel source {source:?}"))?;
                ensure!(
                    owners[index].replace(i).is_none(),
                    "source {source:?} assigned more than once"
                );
            }
        }
        ensure!(
            score.tracks.iter().all(|t| t.routing.sidechain.is_none()),
            "sidechain belongs on an explicit channel/bus; source your track into that channel"
        );
        let tracks = score
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| edges(&t.routing, owners[i], &names))
            .collect::<Result<Vec<_>>>()?;
        let mut nodes = score
            .mixer
            .iter()
            .map(|node| {
                Ok(PreparedNode {
                    controls: node.controls.clone(),
                    detector: node
                        .controls
                        .routing
                        .sidechain
                        .as_ref()
                        .map(|name| {
                            if let Some(&index) = track_names.get(name) {
                                Ok(Detector::Track(index))
                            } else if let Some(&index) = names.get(name) {
                                Ok(Detector::Node(index))
                            } else {
                                anyhow::bail!("unknown sidechain source {name:?}")
                            }
                        })
                        .transpose()?,
                    automation: {
                        ensure!(
                            node.controls.automation.len() <= 32,
                            "maximum 32 automation lanes per mixer node"
                        );
                        node.controls
                            .automation
                            .iter()
                            .map(|a| {
                                let target = match a.parameter.as_str() {
                                    "gain" => Target::Gain,
                                    "pan" => Target::Pan,
                                    "filter" => Target::Filter,
                                    other => {
                                        let (effect, argument) = other
                                            .strip_prefix("effect.")
                                            .and_then(|s| s.split_once('.'))
                                            .context("invalid mixer automation target")?;
                                        let effect = effect
                                            .parse::<usize>()?
                                            .checked_sub(1)
                                            .context("effect index starts at 1")?;
                                        let argument = argument
                                            .parse::<usize>()?
                                            .checked_sub(1)
                                            .context("argument index starts at 1")?;
                                        ensure!(
                                            node.controls
                                                .effects
                                                .get(effect)
                                                .is_some_and(|e| argument < e.args.len()),
                                            "unknown mixer effect/argument"
                                        );
                                        Target::Effect(effect, argument)
                                    }
                                };
                                Ok(PreparedAutomation {
                                    target,
                                    points: a.points.clone(),
                                    step: a.step,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?
                    },
                    effects: vec![],
                    edges: edges(&node.controls.routing, None, &names)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            tracks.iter().map(Vec::len).sum::<usize>()
                + nodes.iter().map(|n| n.edges.len()).sum::<usize>()
                <= 512,
            "mixer edge limit 512"
        );
        let mut successors = vec![Vec::new(); nodes.len()];
        for (i, node) in nodes.iter().enumerate() {
            for edge in &node.edges {
                if let Destination::Node(j) = edge.destination {
                    successors[i].push(j);
                }
            }
            if let Some(Detector::Node(j)) = node.detector {
                successors[j].push(i);
            }
        }
        let mut indegree = vec![0usize; nodes.len()];
        for targets in &successors {
            for &j in targets {
                indegree[j] += 1;
            }
        }
        let mut ready: BTreeSet<_> = indegree
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(i))
            .collect();
        let mut order = Vec::with_capacity(nodes.len());
        while let Some(i) = ready.pop_first() {
            order.push(i);
            for &j in &successors[i] {
                indegree[j] -= 1;
                if indegree[j] == 0 {
                    ready.insert(j);
                }
            }
        }
        ensure!(
            order.len() == nodes.len(),
            "mixer routing/sidechain cycle: feedback is unsupported"
        );
        let effect_count: usize = nodes.iter().map(|n| n.controls.effects.len()).sum();
        ensure!(effect_count <= 256, "project effect limit 256");
        let estimated_bytes: usize = nodes
            .iter()
            .flat_map(|n| &n.controls.effects)
            .map(|e| match e.kind.as_str() {
                "delay" => (e.args[0] as f64 * score.rate as f64).ceil() as usize * 8,
                "reverb" => [0.0297, 0.0371, 0.0411, 0.0437]
                    .iter()
                    .map(|t| (t * score.rate as f64) as usize * 8)
                    .sum(),
                _ => 0,
            })
            .sum();
        ensure!(
            estimated_bytes <= 64 * 1024 * 1024,
            "mixer effect state exceeds 64 MiB"
        );
        for node in &mut nodes {
            node.effects =
                dsp::effects_bypassed(&node.controls.effects, &node.controls.bypassed, score.rate);
        }
        // A soloed bus/channel includes its upstream sources. All downstream buses
        // continue processing; unrelated tracks cannot leak through pre-fader sends.
        let mut reaches_solo: Vec<_> = nodes.iter().map(|n| n.controls.solo).collect();
        for &i in order.iter().rev() {
            let upstream = nodes[i].edges.iter().any(|e| match e.destination {
                Destination::Node(j) => reaches_solo[j],
                Destination::Master => false,
            });
            reaches_solo[i] |= upstream;
        }
        let any_solo = reaches_solo.iter().any(|&s| s) || score.tracks.iter().any(|t| t.solo);
        let audible = tracks
            .iter()
            .zip(&score.tracks)
            .map(|(edges, track)| {
                !any_solo
                    || track.solo
                    || edges.iter().any(|e| match e.destination {
                        Destination::Node(j) => reaches_solo[j],
                        Destination::Master => false,
                    })
            })
            .collect();
        Ok(Some(Self {
            nodes,
            order,
            inputs: vec![[0.; 2]; names.len()],
            track_outputs: vec![[0.; 2]; tracks.len()],
            outputs: vec![[0.; 2]; names.len()],
            tracks,
            audible,
            names: score
                .mixer
                .iter()
                .map(|n| n.controls.name.clone())
                .collect(),
            meters: vec![Meter::default(); names.len()],
        }))
    }
    pub fn track_audible(&self, index: usize) -> bool {
        self.audible[index]
    }
    pub fn begin_frame(&mut self) {
        self.inputs.fill([0.; 2]);
        self.track_outputs.fill([0.; 2]);
        self.outputs.fill([0.; 2]);
    }
    pub fn track(&mut self, index: usize, pre: [f32; 2], post: [f32; 2], master: &mut [f32; 2]) {
        self.track_outputs[index] = post;
        emit(&self.tracks[index], pre, post, &mut self.inputs, master);
    }
    pub fn process(&mut self, time: f64, master: &mut [f32; 2]) -> Result<()> {
        for &i in &self.order {
            let node = &mut self.nodes[i];
            if node.controls.mute {
                continue;
            }
            let mut gain = node.controls.gain;
            let mut pan = node.controls.pan;
            for automation in &node.automation {
                let evaluate = if automation.step {
                    dsp::automation_step
                } else {
                    dsp::automation
                };
                let value = evaluate(time, &automation.points, 0.);
                match automation.target {
                    Target::Gain => gain = value,
                    Target::Pan => pan = value,
                    Target::Filter => {
                        for effect in &mut node.effects {
                            effect.automate_filter(value);
                        }
                    }
                    Target::Effect(effect, argument) => {
                        node.effects[effect].automate_argument(argument, value)
                    }
                }
            }
            let detector = match node.detector {
                Some(Detector::Track(j)) => Some(self.track_outputs[j]),
                Some(Detector::Node(j)) => Some(self.outputs[j]),
                None => None,
            };
            let mut pre = self.inputs[i];
            for effect in &mut node.effects {
                pre = effect.process_with_detector(pre, detector);
                self.meters[i].reduction_db = self.meters[i].reduction_db.max(effect.reduction_db);
            }
            pre = balance(pre, pan);
            let post = pre.map(|v| v * gain);
            ensure!(
                post.iter().all(|v| v.is_finite()),
                "nonfinite mixer output on {:?}",
                node.controls.name
            );
            self.outputs[i] = post;
            let peak = post[0].abs().max(post[1].abs());
            self.meters[i].peak = self.meters[i].peak.max(peak);
            if peak > 1. {
                self.meters[i].clipped_frames += 1;
            }
            emit(&node.edges, pre, post, &mut self.inputs, master);
        }
        Ok(())
    }
    pub fn restore_state(&mut self, state: &Self) {
        for (node, saved) in self.nodes.iter_mut().zip(&state.nodes) {
            node.effects.clone_from(&saved.effects);
        }
        self.inputs.fill([0.; 2]);
    }
    pub fn state_bytes(&self) -> usize {
        self.nodes
            .iter()
            .flat_map(|n| &n.effects)
            .map(Processor::state_bytes)
            .sum()
    }
    pub fn latency_frames(&self) -> usize {
        0
    }
}
