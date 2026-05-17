use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use super::{LogSource, QuantizeEditState, TuiState, VoiceEditState};
use crate::quantizer::Scale;

const SCOPE_COLS: usize = 1024;

pub fn run(state: Arc<TuiState>) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    enable_raw_mode()?;

    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &state);
    let _ = ratatui::restore();

    disable_raw_mode()?;
    stdout.execute(LeaveAlternateScreen)?;

    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    state: &Arc<TuiState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut scatter = false;
    let mut show_reference = true;
    let frame_duration = Duration::from_millis(33);

    loop {
        if !state.running.load(Ordering::Acquire) {
            return Ok(());
        }

        terminal.draw(|frame| {
            render(frame, state, scatter, show_reference);
        })?;

        if event::poll(frame_duration)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let in_voice_edit = state.voice_edit.read().ok().map(|e| e.is_some()).unwrap_or(false);
                    let in_quantize_edit = state.quantize_edit.read().ok().map(|e| e.is_some()).unwrap_or(false);
                    if in_quantize_edit {
                        match key.code {
                            KeyCode::Char('Q') | KeyCode::Esc => {
                                let _ = state.quantize_edit.write().map(|mut e| *e = None);
                            }
                            KeyCode::Left => cycle_scale(state, -1),
                            KeyCode::Right => cycle_scale(state, 1),
                            KeyCode::Tab => cycle_chain_for_quantize(state),
                            KeyCode::Char('+') | KeyCode::Char('=') => adjust_root(state, 1),
                            KeyCode::Char('-') => adjust_root(state, -1),
                            _ => {}
                        }
                    } else if in_voice_edit {
                        match key.code {
                            KeyCode::Char('e') | KeyCode::Esc => {
                                let _ = state.voice_edit.write().map(|mut e| *e = None);
                            }
                            KeyCode::Up => shift_voice_selection(state, -1),
                            KeyCode::Down => shift_voice_selection(state, 1),
                            KeyCode::Tab => cycle_chain_selection(state),
                            KeyCode::Char('+') | KeyCode::Char('=') => adjust_pitch(state, 1),
                            KeyCode::Char('-') => adjust_pitch(state, -1),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                state.running.store(false, Ordering::Release);
                                return Ok(());
                            }
                            KeyCode::Char('s') => scatter = !scatter,
                            KeyCode::Char('r') => show_reference = !show_reference,
                            KeyCode::Char('e') => enter_voice_edit(state),
                            KeyCode::Char('Q') => enter_quantize_edit(state),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn enter_voice_edit(state: &Arc<TuiState>) {
    let idx = state.chains.iter().position(|c| c.present).unwrap_or(0);
    let _ = state.voice_edit.write().map(|mut e| {
        *e = Some(VoiceEditState {
            chain_idx: idx,
            voice_idx: 0,
        })
    });
}

fn shift_voice_selection(state: &Arc<TuiState>, delta: isize) {
    let _ = state.voice_edit.write().map(|mut opt| {
        if let Some(ref mut edit) = *opt {
            let pitches = state.chains[edit.chain_idx].gate_voice_pitches.read().ok();
            if let Some(pitches) = pitches {
                let len = pitches.len();
                if len > 0 {
                    let new = (edit.voice_idx as isize + delta).rem_euclid(len as isize) as usize;
                    edit.voice_idx = new;
                }
            }
        }
    });
}

fn cycle_chain_selection(state: &Arc<TuiState>) {
    let _ = state.voice_edit.write().map(|mut opt| {
        if let Some(ref mut edit) = *opt {
            let present: Vec<usize> = state
                .chains
                .iter()
                .enumerate()
                .filter(|(_, c)| c.present)
                .map(|(i, _)| i)
                .collect();
            if present.len() > 1 {
                let cur_pos = present.iter().position(|&i| i == edit.chain_idx).unwrap_or(0);
                let next = (cur_pos + 1) % present.len();
                edit.chain_idx = present[next];
                edit.voice_idx = 0;
            }
        }
    });
}

fn adjust_pitch(state: &Arc<TuiState>, delta: i8) {
    let edit = match state.voice_edit.read().ok().and_then(|e| *e) {
        Some(e) => e,
        None => return,
    };
    if let Ok(mut pitches) = state.chains[edit.chain_idx].gate_voice_pitches.write() {
        if let Some(p) = pitches.get_mut(edit.voice_idx) {
            let new = (*p as i16 + delta as i16).clamp(0, 127) as u8;
            *p = new;
        }
    }
}

fn enter_quantize_edit(state: &Arc<TuiState>) {
    let idx = state.chains.iter().position(|c| c.present).unwrap_or(0);
    let _ = state.quantize_edit.write().map(|mut e| {
        *e = Some(QuantizeEditState { chain_idx: idx })
    });
}

fn cycle_chain_for_quantize(state: &Arc<TuiState>) {
    let _ = state.quantize_edit.write().map(|mut opt| {
        if let Some(ref mut edit) = *opt {
            let present: Vec<usize> = state
                .chains
                .iter()
                .enumerate()
                .filter(|(_, c)| c.present)
                .map(|(i, _)| i)
                .collect();
            if present.len() > 1 {
                let cur_pos = present.iter().position(|&i| i == edit.chain_idx).unwrap_or(0);
                edit.chain_idx = present[(cur_pos + 1) % present.len()];
            }
        }
    });
}

fn cycle_scale(state: &Arc<TuiState>, delta: isize) {
    let edit = match state.quantize_edit.read().ok().and_then(|e| *e) {
        Some(e) => e,
        None => return,
    };
    let quant_arc = get_quantizer(state, edit.chain_idx);
    if let Some(quant) = quant_arc {
        let mut lock = match quant.write() {
            Ok(l) => l,
            Err(_) => return,
        };
        let all = Scale::all();
        let current = lock.as_ref().map(|q| q.scale).unwrap_or(Scale::Unquantized);
        let pos = all.iter().position(|&s| s == current).unwrap_or(0);
        let new_idx = (pos as isize + delta).rem_euclid(all.len() as isize) as usize;
        let new_scale = all[new_idx];
        if new_scale == Scale::Unquantized {
            *lock = None;
        } else {
            let root = lock.as_ref().map(|q| q.root_note).unwrap_or(60);
            *lock = Some(crate::quantizer::ScaleQuantizer {
                scale: new_scale,
                root_note: root,
            });
        }
    }
}

fn adjust_root(state: &Arc<TuiState>, delta: i8) {
    let edit = match state.quantize_edit.read().ok().and_then(|e| *e) {
        Some(e) => e,
        None => return,
    };
    let quant_arc = get_quantizer(state, edit.chain_idx);
    if let Some(quant) = quant_arc {
        let mut lock = match quant.write() {
            Ok(l) => l,
            Err(_) => return,
        };
        if let Some(ref mut q) = *lock {
            q.root_note = (q.root_note as i16 + delta as i16).clamp(0, 127) as u8;
        }
    }
}

fn get_quantizer(
    state: &Arc<TuiState>,
    chain_idx: usize,
) -> Option<&Arc<RwLock<Option<crate::quantizer::ScaleQuantizer>>>> {
    match chain_idx {
        0 => Some(&state.quantizer_a),
        1 => state.quantizer_b.as_ref(),
        _ => None,
    }
}

fn render(frame: &mut Frame, state: &Arc<TuiState>, scatter: bool, show_reference: bool) {
    let area = frame.area();

    let header_h = 1;
    let status_h = 1;
    let voices_h = 4;
    let params_h = 3;
    let bottom_h = params_h + voices_h + status_h;
    let scope_min_h = 12;

    if area.height < header_h + scope_min_h + bottom_h {
        let msg = format!(
            "Terminal too small — need at least {} rows",
            header_h + scope_min_h + bottom_h
        );
        let para = Paragraph::new(msg).style(Style::default().fg(Color::Red));
        frame.render_widget(para, area);
        return;
    }

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Min(scope_min_h),
            Constraint::Length(bottom_h),
        ])
        .split(area);

    render_header(frame, state, vert[0]);
    render_body(frame, state, vert[1], scatter, show_reference);
    render_bottom(frame, state, vert[2]);
}

fn render_header(frame: &mut Frame, state: &Arc<TuiState>, area: Rect) {
    let tick = state.tick.load(Ordering::Relaxed);
    let left = Span::styled(
        " Crystallized Time Monitor ",
        Style::default().fg(Color::White),
    );
    let right_text = format!("tick: {} ", tick);
    let right = Span::styled(right_text, Style::default().fg(Color::DarkGray));
    let line = Line::from(vec![left, right]);
    let block = Block::default().style(Style::default().bg(Color::Rgb(20, 20, 40)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(line), inner);
}

fn render_body(
    frame: &mut Frame,
    state: &Arc<TuiState>,
    area: Rect,
    scatter: bool,
    show_reference: bool,
) {
    let log_width = 32;
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(40),
            Constraint::Length(log_width),
        ])
        .split(area);

    render_scope(frame, state, horiz[0], scatter, show_reference);
    render_event_log(frame, state, horiz[1]);
}

fn render_scope(
    frame: &mut Frame,
    state: &Arc<TuiState>,
    area: Rect,
    scatter: bool,
    _show_reference: bool,
) {
    let bufs = match state.scope_bufs.read() {
        Ok(b) => b,
        Err(_) => return,
    };

    let n_a = bufs[0].len();
    let n_b = bufs[1].len();
    let n = SCOPE_COLS.min(n_a.max(n_b));

    if n < 2 {
        let block = Block::default()
            .title(" Magnetization Scope ")
            .borders(Borders::ALL);
        let msg = "Waiting for data...";
        frame.render_widget(Paragraph::new(msg).block(block), area);
        return;
    }

    let mut data_a: Vec<(f64, f64)> = Vec::with_capacity(n);
    let mut data_b: Vec<(f64, f64)> = Vec::with_capacity(n);

    let skip_a = bufs[0].len().saturating_sub(n);
    let skip_b = bufs[1].len().saturating_sub(n);

    let y_min = -1.1f64;
    let y_max = 1.1f64;

    if state.chains[0].present {
        for (idx, val) in bufs[0].iter().skip(skip_a).enumerate() {
            let x = idx as f64;
            data_a.push((x, *val));
        }
    } else {
        for idx in 0..n {
            data_a.push((idx as f64, 0.0));
        }
    }

    if state.chains[1].present {
        for (idx, val) in bufs[1].iter().skip(skip_b).enumerate() {
            let x = idx as f64;
            data_b.push((x, *val));
        }
    } else {
        for idx in 0..n {
            data_b.push((idx as f64, 0.0));
        }
    }

    let mut datasets = Vec::new();

    if state.chains[0].present {
        let ds = Dataset::default()
            .name("A")
            .marker(if scatter {
                symbols::Marker::Dot
            } else {
                symbols::Marker::Braille
            })
            .graph_type(if scatter {
                ratatui::widgets::GraphType::Scatter
            } else {
                ratatui::widgets::GraphType::Line
            })
            .style(Style::default().fg(Color::Green))
            .data(&data_a);
        datasets.push(ds);
    }

    if state.chains[1].present {
        let ds = Dataset::default()
            .name("B")
            .marker(if scatter {
                symbols::Marker::Dot
            } else {
                symbols::Marker::Braille
            })
            .graph_type(if scatter {
                ratatui::widgets::GraphType::Scatter
            } else {
                ratatui::widgets::GraphType::Line
            })
            .style(Style::default().fg(Color::Yellow))
            .data(&data_b);
        datasets.push(ds);
    }

    let top_label = if !state.chains[0].present && !state.chains[1].present {
        " Magnetization Scope (no chains) "
    } else if state.chains[0].present && state.chains[1].present {
        " Magnetization Scope — A (green)  B (yellow) "
    } else if state.chains[0].present {
        " Magnetization Scope — A "
    } else {
        " Magnetization Scope — B "
    };

    let y_labels: Vec<Span> = vec!["-1.0", "0.0", "1.0"]
        .iter()
        .map(|s| Span::styled(*s, Style::default().fg(Color::DarkGray)))
        .collect();

    let chart = Chart::new(datasets)
        .block(Block::default().title(top_label).borders(Borders::ALL))
        .x_axis(
            Axis::default()
                .title(Span::styled("tick", Style::default().fg(Color::DarkGray)))
                .bounds([0.0, n.saturating_sub(1) as f64])
                .style(Style::default().fg(Color::DarkGray))
                .labels(
                    vec![
                        Span::styled("0", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("{}", n.saturating_sub(1)),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ],
                ),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("\u{27e8}\u{03c3}z\u{27e9}", Style::default().fg(Color::DarkGray)))
                .bounds([y_min, y_max])
                .style(Style::default().fg(Color::DarkGray))
                .labels(y_labels),
        );

    frame.render_widget(chart, area);
}

fn render_event_log(frame: &mut Frame, state: &Arc<TuiState>, area: Rect) {
    let log = match state.event_log.read() {
        Ok(l) => l,
        Err(_) => return,
    };

    let inner_h = area.height.saturating_sub(2);
    if inner_h == 0 {
        return;
    }

    let entries: Vec<Line> = log
        .iter()
        .rev()
        .take(inner_h as usize)
        .map(|e| {
            let source_str = match e.source {
                LogSource::Osc => "OSC ",
                LogSource::Midi => "MIDI",
                LogSource::Internal => "INT ",
            };
            let color = match e.source {
                LogSource::Osc => Color::Cyan,
                LogSource::Midi => Color::Magenta,
                LogSource::Internal => Color::LightGreen,
            };
            Line::from(vec![
                Span::styled(source_str, Style::default().fg(color)),
                Span::raw(" "),
                Span::raw(&e.content),
            ])
        })
        .collect();

    let block = Block::default()
        .title(format!(" Event Log ({}) ", log.len()))
        .borders(Borders::ALL);

    frame.render_widget(Paragraph::new(entries).block(block), area);
}

fn render_bottom(frame: &mut Frame, state: &Arc<TuiState>, area: Rect) {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(area);

    render_params(frame, state, vert[0]);
    render_voice_pitches(frame, state, vert[1]);
    render_status(frame, state, vert[2]);
}

fn render_params(frame: &mut Frame, state: &Arc<TuiState>, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for (i, chain) in state.chains.iter().enumerate() {
        if !chain.present {
            continue;
        }
        let label = if i == 0 { "A" } else { "B" };
        let kt = chain.kt.read().ok().map(|g| *g).unwrap_or(0.0);
        let eps = chain.eps.read().ok().map(|g| *g).unwrap_or(0.0);
        let j = chain.j.read().ok().map(|g| *g).unwrap_or(0.0);
        let w = chain.w.read().ok().map(|g| *g).unwrap_or(0.0);
        let mag = chain.magnetization.read().ok().map(|g| *g).unwrap_or(0.0);

        let line = Line::from(vec![
            Span::styled(
                format!("Chain {}: ", label),
                Style::default().fg(Color::LightBlue),
            ),
            Span::styled(format!("kT {:.3}  ", kt), Style::default()),
            Span::styled(format!("\u{03b5} {:.3}  ", eps), Style::default()),
            Span::styled(format!("J {:.3}  ", j), Style::default()),
            Span::styled(format!("\u{03c9} {:.3}  ", w), Style::default()),
            Span::styled(
                format!("\u{27e8}M\u{27e9} {:.3}", mag),
                Style::default().fg(if mag >= 0.0 {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]);
        lines.push(line);
    }

    if let Ok(coupling) = state.coupling.read() {
        if let Some(ref c) = *coupling {
            let shape = c.shape.clone();
            let strength_ab = c.strength_ab;
            let strength_ba = c.strength_ba;
            drop(coupling);
            let line = Line::from(vec![
                Span::styled("Coupling: ", Style::default().fg(Color::LightBlue)),
                Span::raw(shape),
                Span::raw("  A\u{2192}B "),
                Span::styled(format!("{:.2}", strength_ab), Style::default()),
                Span::raw("  B\u{2192}A "),
                Span::styled(format!("{:.2}", strength_ba), Style::default()),
            ]);
            lines.push(line);
        }
    }

    let block = Block::default().borders(Borders::NONE);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_voice_pitches(frame: &mut Frame, state: &Arc<TuiState>, area: Rect) {
    let edit = state.voice_edit.read().ok().and_then(|e| *e);
    let quant_edit = state.quantize_edit.read().ok().and_then(|e| *e);
    let mut lines: Vec<Line> = Vec::new();

    for (i, chain) in state.chains.iter().enumerate() {
        if !chain.present {
            continue;
        }
        let label = if i == 0 { "A" } else { "B" };
        let pitches = chain.gate_voice_pitches.read().ok();
        if let Some(pitches) = pitches {
            let mut spans: Vec<Span> = vec![
                Span::styled(
                    format!("Chain {} voices: ", label),
                    Style::default().fg(Color::LightBlue),
                ),
            ];
            for (v, &pitch) in pitches.iter().enumerate() {
                let is_selected = edit
                    .map(|e| e.chain_idx == i && e.voice_idx == v)
                    .unwrap_or(false);
                let pitch_name = midi_note_name(pitch);
                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };
                spans.push(Span::styled(
                    format!("v{}:{}({}) ", v, pitch, pitch_name),
                    style,
                ));
            }
            lines.push(Line::from(spans));
        }

        let quant = get_quantizer(state, i).and_then(|q| q.read().ok()).and_then(|l| l.clone());
        let is_quant_selected = quant_edit.map(|e| e.chain_idx == i).unwrap_or(false);
        let scale_name = quant.as_ref().map(|q| q.scale.name()).unwrap_or("Unquantized");
        let root_name = quant.as_ref().map(|q| midi_note_name(q.root_note)).unwrap_or_else(|| "C4".into());
        let style = if is_quant_selected {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::Cyan)
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  Scale {}: ", label), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}({})", scale_name, root_name), style),
        ]));
    }

    if let Some(ref _edit_state) = edit {
        lines.push(Line::from(vec![Span::styled(
            "EDIT: Up/Dn=voice  Tab=chain  +/-=pitch  e/Esc=done",
            Style::default().fg(Color::DarkGray),
        )]));
    }
    if quant_edit.is_some() {
        lines.push(Line::from(vec![Span::styled(
            "SCALE EDIT: Left/Right=scale  Tab=chain  +/-=root  Q/Esc=done",
            Style::default().fg(Color::DarkGray),
        )]));
    }

    let block = Block::default().borders(Borders::NONE);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn midi_note_name(pitch: u8) -> String {
    static NAMES: &[&str] = &[
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (pitch as i16 / 12) - 1;
    let name = NAMES[(pitch % 12) as usize];
    format!("{}{}", name, octave)
}

fn render_status(frame: &mut Frame, state: &Arc<TuiState>, area: Rect) {
    let walls_a = state.chains[0].wall_count.read().ok().map(|g| *g).unwrap_or(0);
    let walls_b = state.chains[1].wall_count.read().ok().map(|g| *g).unwrap_or(0);

    let mut segments = vec![
        Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
    ];

    if state.chains[0].present {
        segments.push(Span::styled(
            format!("walls A={}  ", walls_a),
            Style::default(),
        ));
    }
    if state.chains[1].present {
        segments.push(Span::styled(
            format!("walls B={}  ", walls_b),
            Style::default(),
        ));
    }
    segments.push(Span::styled(
        format!("BPM={:.0}  ", state.bpm),
        Style::default(),
    ));
    segments.push(Span::styled(
        "Press q to quit  s:scatter  r:ref  e:edit voices  Q:edit scale",
        Style::default().fg(Color::DarkGray),
    ));

    let line = Line::from(segments);
    frame.render_widget(Paragraph::new(line), area);
}
