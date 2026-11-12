//! Format-neutral host contracts and parameter/state handling.
//! No foreign plugin ABI adapter is implemented or advertised by this module.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    Clap,
    Vst3,
    Lv2,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub format: Format,
    pub plugin_id: String,
    pub instance_id: String,
}
#[derive(Clone, Debug)]
pub struct Parameter {
    pub id: String,
    pub name: String,
    pub minimum: f64,
    pub maximum: f64,
    pub writable: bool,
    pub automatable: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Code,
    Automation,
    Midi,
    PluginGui,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiBinding {
    pub channel: u8,
    pub controller: u8,
    pub parameter: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub identity: Identity,
    pub parameters: BTreeMap<String, f64>,
    pub midi: Vec<MidiBinding>,
    pub opaque: Vec<u8>,
}
#[derive(Clone, Copy, Default)]
pub struct Capabilities {
    pub instrument: bool,
    pub original_gui: bool,
    pub headless: bool,
    pub midi_input: bool,
    pub midi_output: bool,
    pub offline: bool,
}
#[derive(Clone, Copy)]
pub struct MidiEvent {
    pub frame: u32,
    pub bytes: [u8; 3],
}
/// An adapter must perform preparation/discovery/state I/O off the audio path.
/// Process must not allocate, block, log or open files; block/event storage is owned
/// by the host. GUI requests must be marshalled to the platform's main UI thread,
/// targeting this identity, not a newly constructed instance.
pub trait Backend {
    fn identity(&self) -> &Identity;
    fn capabilities(&self) -> Capabilities;
    fn parameters(&self) -> Result<Vec<Parameter>>;
    fn set_parameter(&mut self, id: &str, value: f64) -> Result<()>;
    fn parameter(&self, id: &str) -> Result<f64>;
    fn prepare(&mut self, rate: u32, maximum_block: usize, offline: bool) -> Result<()>;
    fn process(
        &mut self,
        input: &[[f32; 2]],
        output: &mut [[f32; 2]],
        midi: &[MidiEvent],
    ) -> Result<()>;
    fn latency_frames(&self) -> usize;
    fn state(&mut self) -> Result<Vec<u8>>;
    fn restore(&mut self, state: &[u8]) -> Result<()>;
    fn request_editor(&mut self, open: bool) -> Result<()>;
}
pub struct Instance<B: Backend> {
    backend: B,
    parameters: BTreeMap<String, Parameter>,
    values: BTreeMap<String, f64>,
    midi: Vec<MidiBinding>,
    learning: Option<String>,
    maximum_block: usize,
}
impl<B: Backend> Instance<B> {
    pub fn new(backend: B) -> Result<Self> {
        let identity = backend.identity();
        ensure!(
            !identity.plugin_id.is_empty()
                && identity.plugin_id.len() <= 4096
                && !identity.instance_id.is_empty()
                && identity.instance_id.len() <= 256,
            "invalid plugin identity"
        );
        let mut parameters = BTreeMap::new();
        let mut values = BTreeMap::new();
        let discovered = backend.parameters()?;
        ensure!(discovered.len() <= 16384, "plugin parameter limit 16384");
        for parameter in discovered {
            ensure!(
                !parameter.id.is_empty()
                    && parameter.id.len() <= 1024
                    && parameter.name.len() <= 4096,
                "plugin parameter metadata exceeds bounds"
            );
            ensure!(
                parameter.minimum.is_finite()
                    && parameter.maximum.is_finite()
                    && (parameter.minimum < parameter.maximum
                        || (!parameter.writable && parameter.minimum == parameter.maximum)),
                "invalid plugin parameter range"
            );
            let value = backend.parameter(&parameter.id)?;
            ensure!(
                value.is_finite() && (parameter.minimum..=parameter.maximum).contains(&value),
                "invalid initial plugin value"
            );
            ensure!(
                !parameters.contains_key(&parameter.id),
                "duplicate plugin parameter ID"
            );
            values.insert(parameter.id.clone(), value);
            parameters.insert(parameter.id.clone(), parameter);
        }
        Ok(Self {
            backend,
            parameters,
            values,
            midi: vec![],
            learning: None,
            maximum_block: 0,
        })
    }
    pub fn set(&mut self, id: &str, value: f64, origin: Origin) -> Result<()> {
        let parameter = self
            .parameters
            .get(id)
            .context("plugin does not expose this parameter")?;
        ensure!(
            origin == Origin::PluginGui || parameter.writable,
            "plugin parameter is read-only"
        );
        ensure!(
            origin != Origin::Automation || parameter.automatable,
            "plugin parameter is not automatable"
        );
        ensure!(
            value.is_finite() && (parameter.minimum..=parameter.maximum).contains(&value),
            "plugin parameter outside its declared range"
        );
        // A plugin-GUI notification already changed this same backend instance.
        // Never echo it back and create a parameter-feedback loop.
        if origin != Origin::PluginGui {
            self.backend.set_parameter(id, value)?;
        }
        self.values.insert(id.into(), value);
        Ok(())
    }
    pub fn values(&self) -> &BTreeMap<String, f64> {
        &self.values
    }
    pub fn parameters(&self) -> &BTreeMap<String, Parameter> {
        &self.parameters
    }
    pub fn learn(&mut self, id: &str) -> Result<()> {
        ensure!(
            self.parameters.get(id).is_some_and(|p| p.writable),
            "cannot learn an unavailable/read-only parameter"
        );
        self.learning = Some(id.into());
        Ok(())
    }
    pub fn controller(&mut self, channel: u8, controller: u8, value: u8) -> Result<()> {
        ensure!(
            (1..=16).contains(&channel) && controller <= 127 && value <= 127,
            "invalid MIDI CC"
        );
        if let Some(parameter) = self.learning.take() {
            self.midi
                .retain(|b| b.channel != channel || b.controller != controller);
            ensure!(self.midi.len() < 2048, "MIDI mapping limit 2048");
            self.midi.push(MidiBinding {
                channel,
                controller,
                parameter,
            });
        }
        // Control-plane operation, not a realtime MIDI callback.
        let targets: Vec<_> = self
            .midi
            .iter()
            .filter(|b| b.channel == channel && b.controller == controller)
            .map(|b| b.parameter.clone())
            .collect();
        for id in targets {
            let p = &self.parameters[&id];
            let raw = p.minimum + (p.maximum - p.minimum) * value as f64 / 127.;
            self.set(&id, raw, Origin::Midi)?;
        }
        Ok(())
    }
    pub fn prepare(&mut self, rate: u32, maximum_block: usize, offline: bool) -> Result<()> {
        ensure!(
            (8000..=192000).contains(&rate) && (1..=4096).contains(&maximum_block),
            "unsupported plugin processing configuration"
        );
        ensure!(
            !offline || self.backend.capabilities().offline,
            "plugin does not support offline rendering"
        );
        self.backend.prepare(rate, maximum_block, offline)?;
        self.maximum_block = maximum_block;
        Ok(())
    }
    pub fn process(
        &mut self,
        input: &[[f32; 2]],
        output: &mut [[f32; 2]],
        midi: &[MidiEvent],
    ) -> Result<()> {
        output.fill([0.; 2]);
        ensure!(
            input.iter().flatten().all(|v| v.is_finite()),
            "nonfinite plugin input"
        );
        ensure!(
            self.maximum_block > 0
                && input.len() == output.len()
                && output.len() <= self.maximum_block,
            "plugin block exceeds prepared bounds"
        );
        ensure!(
            midi.len() <= 4096 && midi.iter().all(|e| (e.frame as usize) < output.len()),
            "plugin MIDI block exceeds bounds"
        );
        if let Err(error) = self.backend.process(input, output, midi) {
            output.fill([0.; 2]);
            return Err(error);
        }
        if !output.iter().flatten().all(|v| v.is_finite()) {
            output.fill([0.; 2]);
            anyhow::bail!("plugin produced nonfinite audio");
        }
        Ok(())
    }
    pub fn editor(&mut self, open: bool) -> Result<()> {
        ensure!(
            self.backend.capabilities().original_gui,
            "plugin has no supported original GUI"
        );
        self.backend.request_editor(open)
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let opaque = self.backend.state()?;
        ensure!(
            opaque.len() <= MAX_STATE_BYTES,
            "plugin state exceeds 16 MiB"
        );
        Ok(Snapshot {
            version: 1,
            identity: self.backend.identity().clone(),
            parameters: self.values.clone(),
            midi: self.midi.clone(),
            opaque,
        })
    }
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<()> {
        let identity = self.backend.identity();
        ensure!(
            snapshot.version == 1
                && snapshot.identity.format == identity.format
                && snapshot.identity.plugin_id == identity.plugin_id
                && snapshot.identity.instance_id == identity.instance_id,
            "plugin state identity/version mismatch; original state retained"
        );
        ensure!(
            snapshot.opaque.len() <= MAX_STATE_BYTES && snapshot.midi.len() <= 2048,
            "plugin snapshot exceeds bounds"
        );
        for (id, value) in &snapshot.parameters {
            let p = self.parameters.get(id).context(
                "snapshot parameter unavailable; retain snapshot for missing-plugin recovery",
            )?;
            ensure!(
                value.is_finite() && (p.minimum..=p.maximum).contains(value),
                "snapshot parameter outside range"
            );
        }
        for binding in &snapshot.midi {
            ensure!(
                (1..=16).contains(&binding.channel)
                    && binding.controller <= 127
                    && self
                        .parameters
                        .get(&binding.parameter)
                        .is_some_and(|p| p.writable),
                "invalid snapshot MIDI binding"
            );
        }
        self.backend.restore(&snapshot.opaque)?;
        // The adapter owns restoration semantics; re-read its authoritative values.
        for id in self.parameters.keys() {
            self.values.insert(id.clone(), self.backend.parameter(id)?);
        }
        self.midi.clone_from(&snapshot.midi);
        self.learning = None;
        Ok(())
    }
}
pub fn compatibility() -> &'static str {
    "VST3: adapter not implemented; CLAP: adapter not implemented; LV2: adapter not implemented. No original GUI, binary scanning/loading, host audio routing, OS MIDI input/output or plugin latency compensation is operational. Format-neutral contracts are not plugin compatibility."
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        identity: Identity,
        gain: f64,
        open: bool,
        fail: bool,
    }
    impl Backend for Fixture {
        fn identity(&self) -> &Identity {
            &self.identity
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                original_gui: true,
                headless: true,
                offline: true,
                ..Default::default()
            }
        }
        fn parameters(&self) -> Result<Vec<Parameter>> {
            Ok(vec![Parameter {
                id: "gain".into(),
                name: "Gain".into(),
                minimum: 0.,
                maximum: 1.,
                writable: true,
                automatable: true,
            }])
        }
        fn set_parameter(&mut self, id: &str, value: f64) -> Result<()> {
            ensure!(id == "gain", "unknown parameter");
            self.gain = value;
            Ok(())
        }
        fn parameter(&self, _: &str) -> Result<f64> {
            Ok(self.gain)
        }
        fn prepare(&mut self, _: u32, _: usize, _: bool) -> Result<()> {
            Ok(())
        }
        fn process(
            &mut self,
            input: &[[f32; 2]],
            output: &mut [[f32; 2]],
            _: &[MidiEvent],
        ) -> Result<()> {
            ensure!(!self.fail, "fixture processing error");
            for (input, output) in input.iter().zip(output) {
                *output = input.map(|v| v * self.gain as f32);
            }
            Ok(())
        }
        fn latency_frames(&self) -> usize {
            0
        }
        fn state(&mut self) -> Result<Vec<u8>> {
            Ok(self.gain.to_le_bytes().to_vec())
        }
        fn restore(&mut self, data: &[u8]) -> Result<()> {
            self.gain = f64::from_le_bytes(data.try_into()?);
            Ok(())
        }
        fn request_editor(&mut self, open: bool) -> Result<()> {
            self.open = open;
            Ok(())
        }
    }
    fn instance() -> Instance<Fixture> {
        Instance::new(Fixture {
            identity: Identity {
                format: Format::Clap,
                plugin_id: "test-only".into(),
                instance_id: "one".into(),
            },
            gain: 0.5,
            open: false,
            fail: false,
        })
        .unwrap()
    }
    #[test]
    fn interfaces_midi_and_state_share_one_fixture_instance() {
        let mut host = instance();
        host.prepare(48000, 64, true).unwrap();
        host.set("gain", 0.25, Origin::Code).unwrap();
        host.editor(true).unwrap();
        host.backend.gain = 0.75; // Simulated notification from this same instance's GUI.
        host.set("gain", 0.75, Origin::PluginGui).unwrap();
        assert_eq!(host.values()["gain"], host.backend.gain);
        host.learn("gain").unwrap();
        host.controller(1, 7, 127).unwrap();
        assert_eq!(host.backend.gain, 1.);
        let snapshot = host.snapshot().unwrap();
        host.set("gain", 0., Origin::Automation).unwrap();
        host.restore(&snapshot).unwrap();
        host.editor(false).unwrap();
        let mut output = [[0.; 2]; 4];
        host.process(&[[0.5; 2]; 4], &mut output, &[]).unwrap();
        assert_eq!(output, [[0.5; 2]; 4]);
        assert!(!host.backend.open);
    }
    #[test]
    fn invalid_parameters_identity_and_processing_errors_fail_closed() {
        let mut host = instance();
        assert!(host.set("missing", 0., Origin::Code).is_err());
        assert!(host.set("gain", f64::NAN, Origin::Code).is_err());
        let mut snapshot = host.snapshot().unwrap();
        snapshot.identity.plugin_id = "other".into();
        assert!(host.restore(&snapshot).is_err());
        assert_eq!(host.backend.gain, 0.5);
        host.prepare(48000, 64, false).unwrap();
        host.backend.fail = true;
        let mut output = [[1.; 2]; 4];
        assert!(host.process(&[[0.; 2]; 4], &mut output, &[]).is_err());
        assert_eq!(output, [[0.; 2]; 4]);
    }
}
