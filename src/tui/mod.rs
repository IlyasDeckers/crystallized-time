pub mod app;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, RwLock};

use crate::quantizer::ScaleQuantizer;

#[derive(Clone, Copy, Debug)]
pub struct VoiceEditState {
    pub chain_idx: usize,
    pub voice_idx: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct QuantizeEditState {
    pub chain_idx: usize,
}

pub struct TuiState {
    pub tick: AtomicU64,
    pub bpm: f64,
    pub chains: [ChainState; 2],
    pub coupling: RwLock<Option<CouplingInfo>>,
    pub event_log: RwLock<VecDeque<LogEntry>>,
    pub scope_bufs: RwLock<[VecDeque<f64>; 2]>,
    pub scope_buf_cap: usize,
    pub running: Arc<AtomicBool>,
    pub voice_edit: RwLock<Option<VoiceEditState>>,
    pub quantize_edit: RwLock<Option<QuantizeEditState>>,
    pub quantizer_a: Arc<RwLock<Option<ScaleQuantizer>>>,
    pub quantizer_b: Option<Arc<RwLock<Option<ScaleQuantizer>>>>,
}

pub struct ChainState {
    pub present: bool,
    pub kt: RwLock<f64>,
    pub eps: RwLock<f64>,
    pub j: RwLock<f64>,
    pub w: RwLock<f64>,
    pub magnetization: RwLock<f64>,
    pub wall_count: RwLock<u64>,
    #[allow(dead_code)]
    pub gate_rate: RwLock<f64>,
    pub gate_voice_pitches: Arc<RwLock<Vec<u8>>>,
}

pub struct CouplingInfo {
    pub shape: String,
    pub strength_ab: f64,
    pub strength_ba: f64,
}

pub struct LogEntry {
    pub source: LogSource,
    pub content: String,
}

pub enum LogSource {
    Osc,
    Midi,
    Internal,
}

impl TuiState {
    pub fn new(
        bpm: f64,
        running: Arc<AtomicBool>,
        chain_b_present: bool,
        voice_pitches_a: Arc<RwLock<Vec<u8>>>,
        voice_pitches_b: Option<Arc<RwLock<Vec<u8>>>>,
        quantizer_a: Arc<RwLock<Option<ScaleQuantizer>>>,
        quantizer_b: Option<Arc<RwLock<Option<ScaleQuantizer>>>>,
    ) -> Self {
        let cap = 1024;
        let empty_b = Arc::new(RwLock::new(Vec::new()));
        Self {
            tick: AtomicU64::new(0),
            bpm,
            chains: [
                ChainState::new(true, voice_pitches_a),
                ChainState::new(chain_b_present, voice_pitches_b.unwrap_or_else(|| Arc::clone(&empty_b))),
            ],
            coupling: RwLock::new(None),
            event_log: RwLock::new(VecDeque::with_capacity(500)),
            scope_bufs: RwLock::new([
                VecDeque::with_capacity(cap),
                VecDeque::with_capacity(cap),
            ]),
            scope_buf_cap: cap,
            running,
            voice_edit: RwLock::new(None),
            quantize_edit: RwLock::new(None),
            quantizer_a,
            quantizer_b,
        }
    }

    pub fn push_log(&self, source: LogSource, content: String) {
        let mut log = match self.event_log.write() {
            Ok(l) => l,
            Err(_) => return,
        };
        if log.len() >= 500 {
            log.pop_front();
        }
        log.push_back(LogEntry { source, content });
    }
}

impl ChainState {
    fn new(present: bool, gate_voice_pitches: Arc<RwLock<Vec<u8>>>) -> Self {
        Self {
            present,
            kt: RwLock::new(0.0),
            eps: RwLock::new(0.0),
            j: RwLock::new(0.0),
            w: RwLock::new(0.0),
            magnetization: RwLock::new(0.0),
            wall_count: RwLock::new(0),
            gate_rate: RwLock::new(0.0),
            gate_voice_pitches,
        }
    }
}

pub fn spawn(state: Arc<TuiState>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        if let Err(e) = app::run(state) {
            eprintln!("TUI thread exited with error: {}", e);
        }
    })
}
