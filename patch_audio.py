#!/usr/bin/env python3
"""Patch audio.rs with all four DSP improvements (idempotent)."""

with open('src/audio.rs', 'r', encoding='utf-8') as f:
    c = f.read()

orig = c
patches_applied = 0

def patch(old, new, name):
    global c, patches_applied
    if new.split('\n')[0] in c or old not in c:
        if old not in c:
            print(f"  skip {name} (old not found, likely already applied or text changed)")
        else:
            print(f"  skip {name} (already applied)")
        return
    c = c.replace(old, new, 1)
    patches_applied += 1
    print(f"  applied {name}")

# 1. StereoWidth type alias
patch(
    'pub type ClipBuffer = Arc<Mutex<VecDeque<f32>>>;\n\n/// One-shot playback',
    'pub type ClipBuffer = Arc<Mutex<VecDeque<f32>>>;\n/// #4 — Shared master stereo width.\npub type StereoWidth = Arc<Mutex<f32>>;\n\n/// One-shot playback',
    'patch1-StereoWidth-type'
)

# 2. New fields on LayerSynth struct (only if voice_age not already there)
if 'voice_age: [u32; 4],' not in c:
    old = '    formant_freqs: [f32; 3],\n}'
    new = ('    formant_freqs: [f32; 3],\n'
           '    // #1 — Voice stealing: age counter per voice\n'
           '    voice_age: [u32; 4],\n'
           '    // #5 — Doppler portamento overshoot\n'
           '    prev_freq_target: [f32; 4],\n'
           '    freq_doppler_overshoot: [f32; 4],\n'
           '    doppler_decay_counter: [u32; 4],\n}')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch2-LayerSynth-fields")
    else:
        print("  skip patch2 (old not found)")
else:
    print("  skip patch2 (already applied)")

# 3. Field initialization
if 'voice_age: [0u32; 4],' not in c:
    old = ('            vocoder_buzz_phase: 0.0,\n'
           '            formant_freqs: [800.0, 1200.0, 2500.0],\n'
           '        }\n'
           '    }\n\n'
           '    fn update(')
    new = ('            vocoder_buzz_phase: 0.0,\n'
           '            formant_freqs: [800.0, 1200.0, 2500.0],\n'
           '            voice_age: [0u32; 4],\n'
           '            prev_freq_target: [0.0f32; 4],\n'
           '            freq_doppler_overshoot: [0.0f32; 4],\n'
           '            doppler_decay_counter: [0u32; 4],\n'
           '        }\n'
           '    }\n\n'
           '    fn update(')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch3-init")
    else:
        print("  skip patch3 (old not found)")
else:
    print("  skip patch3 (already applied)")

# 4. Voice stealing in update()
if 'steal_oldest_voice()' not in c:
    old = ('            for adsr in &mut self.voice_adsr {\n'
           '                adsr.set_params(att, p.adsr_decay_ms, p.adsr_sustain, rel);\n'
           '                adsr.trigger();\n'
           '            }\n'
           '        }\n'
           '        self.ks.volume')
    new = ('            // #1 — Voice stealing: steal oldest sustaining voice\n'
           '            let stolen = self.steal_oldest_voice();\n'
           '            self.voice_adsr[stolen].set_params(att, p.adsr_decay_ms, p.adsr_sustain, rel);\n'
           '            self.voice_adsr[stolen].trigger();\n'
           '            self.voice_age[stolen] = 0;\n'
           '        }\n'
           '        self.ks.volume')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch4-voice-stealing")
    else:
        print("  skip patch4 (old not found)")
else:
    print("  skip patch4 (already applied)")

# 5. steal_oldest_voice method
if 'fn steal_oldest_voice' not in c:
    old = '    /// Render one stereo sample for this layer (no master effects yet).\n    fn next_sample('
    new = ('    /// #1 — Voice stealing helper: returns index of voice sustaining longest.\n'
           '    fn steal_oldest_voice(&self) -> usize {\n'
           '        self.voice_age.iter().enumerate()\n'
           '            .max_by_key(|&(_, &a)| a).map(|(i, _)| i).unwrap_or(0)\n'
           '    }\n\n'
           '    /// Render one stereo sample for this layer (no master effects yet).\n'
           '    fn next_sample(')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch5-steal-method")
    else:
        print("  skip patch5 (old not found)")
else:
    print("  skip patch5 (already applied)")

# 6. Doppler + voice_age in synth_additive
if 'freq_doppler_overshoot' not in c or 'freq_out' not in c:
    old = ('                self.freq_smooth[i] += self.freq_smooth_rate * (target_freq - self.freq_smooth[i]);\n'
           '                self.amp_smooth[i]  += 0.005 * (target_amp - self.amp_smooth[i]);\n'
           '                self.oscs[i].freq = self.freq_smooth[i];\n'
           '                let env = self.voice_adsr[i].next_sample();\n'
           '                let sig = self.oscs[i].next_sample() * self.amp_smooth[i] * gain * env;')
    new = ('                // #5 — Doppler-effect portamento overshoot\n'
           '                let delta = target_freq - self.prev_freq_target[i];\n'
           '                if delta.abs() > 0.5 {\n'
           '                    self.freq_doppler_overshoot[i] =\n'
           '                        delta * 0.04 * (-(self.doppler_decay_counter[i] as f32) * 0.002).exp();\n'
           '                    self.doppler_decay_counter[i] += 1;\n'
           '                } else {\n'
           '                    self.doppler_decay_counter[i] = 0;\n'
           '                    self.freq_doppler_overshoot[i] *= 0.95;\n'
           '                }\n'
           '                self.prev_freq_target[i] = target_freq;\n'
           '                self.freq_smooth[i] += self.freq_smooth_rate * (target_freq - self.freq_smooth[i]);\n'
           '                let freq_out = (self.freq_smooth[i] + self.freq_doppler_overshoot[i]).max(10.0);\n'
           '                self.amp_smooth[i]  += 0.005 * (target_amp - self.amp_smooth[i]);\n'
           '                self.oscs[i].freq = freq_out;\n'
           '                // #1 — increment voice age each sample\n'
           '                self.voice_age[i] = self.voice_age[i].saturating_add(1);\n'
           '                let env = self.voice_adsr[i].next_sample();\n'
           '                let sig = self.oscs[i].next_sample() * self.amp_smooth[i] * gain * env;')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch6-doppler")
    else:
        print("  skip patch6 (old not found)")
else:
    print("  skip patch6 (already applied)")

# 7. stereo_width field in SynthState
if 'stereo_width: f32,' not in c:
    old = '    master_volume: f32,\n    // Sidechain duck (KS trigger ducks reverb/delay output)\n    sidechain_duck: f32,'
    new = ('    master_volume: f32,\n'
           '    /// #4 — Mid/side stereo width after limiter (0=mono, 1=unity, 3=hyper-wide).\n'
           '    stereo_width: f32,\n'
           '    // Sidechain duck (KS trigger ducks reverb/delay output)\n'
           '    sidechain_duck: f32,')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch7-stereo_width-field")
    else:
        print("  skip patch7 (old not found)")
else:
    print("  skip patch7 (already applied)")

# 8. Initialize stereo_width
if 'stereo_width: 1.0,' not in c:
    old = '            master_volume: 0.7,\n            sidechain_duck: 1.0,'
    new = '            master_volume: 0.7,\n            stereo_width: 1.0,\n            sidechain_duck: 1.0,'
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch8-stereo_width-init")
    else:
        print("  skip patch8 (old not found)")
else:
    print("  skip patch8 (already applied)")

# 9. Mid/side after limiter
if 'lo_lim, ro_lim' not in c:
    old = ('        let (lo_raw, ro_raw) = self.limiter.process(lrev, rrev);\n\n'
           '        // Final NaN guard\n'
           '        let mut lo = if lo_raw.is_finite() { lo_raw } else { 0.0 };\n'
           '        let mut ro = if ro_raw.is_finite() { ro_raw } else { 0.0 };')
    new = ('        let (lo_lim, ro_lim) = self.limiter.process(lrev, rrev);\n\n'
           '        // #4 — Master stereo width: mid/side matrix after limiter.\n'
           '        let (lo_raw, ro_raw) = {\n'
           '            let w = self.stereo_width.clamp(0.0, 3.0);\n'
           '            let mid  = (lo_lim + ro_lim) * 0.5;\n'
           '            let side = (lo_lim - ro_lim) * 0.5 * w;\n'
           '            let norm = 1.0 / (0.5 + w * w * 0.5f32).sqrt();\n'
           '            ((mid + side) * norm, (mid - side) * norm)\n'
           '        };\n\n'
           '        // Final NaN guard\n'
           '        let mut lo = if lo_raw.is_finite() { lo_raw } else { 0.0 };\n'
           '        let mut ro = if ro_raw.is_finite() { ro_raw } else { 0.0 };')
    if old in c:
        c = c.replace(old, new, 1)
        patches_applied += 1
        print("  applied patch9-mid-side")
    else:
        print("  skip patch9 (old not found)")
else:
    print("  skip patch9 (already applied)")

# 10 & 11. stereo_width param and wiring in AudioEngine::start
if 'stereo_width: StereoWidth,' not in c:
    # Find snippet_pb param
    old10 = ('        snippet_pb: SharedSnippetPlayback,\n'
             '    ) -> anyhow::Result<(Self, u32)> {')
    new10 = ('        snippet_pb: SharedSnippetPlayback,\n'
             '        stereo_width: StereoWidth,\n'
             '    ) -> anyhow::Result<(Self, u32)> {')
    if old10 in c:
        c = c.replace(old10, new10, 1)
        patches_applied += 1
        print("  applied patch10-start-param")
    else:
        print("  skip patch10 (old not found)")
else:
    print("  skip patch10 (already applied)")

# Wire sw into callbacks
if 'let sw = stereo_width.clone();' not in c:
    # F32 branch
    old_f32 = ('            SampleFormat::F32 => {\n'
               '                let ss = synth.clone();\n'
               '                device.build_output_stream(\n'
               '                    &stream_config,\n'
               '                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {\n'
               '                        let latest = drain(&params_rx);\n'
               '                        let mut state = ss.lock();\n'
               '                        for i in 0..3 { if let Some(p) = latest[i].clone() { state.update_params(i, p); } }\n'
               '                        state.render(data);\n'
               '                    },\n'
               '                    |err| log::error!("Audio stream error: {err}"), None)?\n'
               '            }\n'
               '            _ => {\n'
               '                // For I16/U16: convert via f32 buffer, same drain logic\n'
               '                let ss = synth.clone();\n'
               '                device.build_output_stream(\n'
               '                    &stream_config,\n'
               '                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {\n'
               '                        let latest = drain(&params_rx);\n'
               '                        let mut state = ss.lock();\n'
               '                        for i in 0..3 { if let Some(p) = latest[i].clone() { state.update_params(i, p); } }\n'
               '                        state.render(data);\n'
               '                    },\n'
               '                    |err| log::error!("Audio stream error: {err}"), None)?\n'
               '            }\n'
               '        };')
    new_f32 = ('            SampleFormat::F32 => {\n'
               '                let ss = synth.clone();\n'
               '                let sw = stereo_width.clone();\n'
               '                device.build_output_stream(\n'
               '                    &stream_config,\n'
               '                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {\n'
               '                        let latest = drain(&params_rx);\n'
               '                        let mut state = ss.lock();\n'
               '                        if let Some(w) = sw.try_lock() { state.stereo_width = *w; }\n'
               '                        for i in 0..3 { if let Some(p) = latest[i].clone() { state.update_params(i, p); } }\n'
               '                        state.render(data);\n'
               '                    },\n'
               '                    |err| log::error!("Audio stream error: {err}"), None)?\n'
               '            }\n'
               '            _ => {\n'
               '                // For I16/U16: convert via f32 buffer, same drain logic\n'
               '                let ss = synth.clone();\n'
               '                let sw = stereo_width.clone();\n'
               '                device.build_output_stream(\n'
               '                    &stream_config,\n'
               '                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {\n'
               '                        let latest = drain(&params_rx);\n'
               '                        let mut state = ss.lock();\n'
               '                        if let Some(w) = sw.try_lock() { state.stereo_width = *w; }\n'
               '                        for i in 0..3 { if let Some(p) = latest[i].clone() { state.update_params(i, p); } }\n'
               '                        state.render(data);\n'
               '                    },\n'
               '                    |err| log::error!("Audio stream error: {err}"), None)?\n'
               '            }\n'
               '        };')
    if old_f32 in c:
        c = c.replace(old_f32, new_f32, 1)
        patches_applied += 1
        print("  applied patch11-sw-wire")
    else:
        print("  skip patch11 (old not found - may be linter variant)")
        # Try to find what's actually there
        idx = c.find('SampleFormat::F32')
        if idx >= 0:
            print(f"  Context: {repr(c[idx:idx+300])}")
else:
    print("  skip patch11 (already applied)")

if c != orig:
    with open('src/audio.rs', 'w', encoding='utf-8') as f:
        f.write(c)
    print(f"Done: {patches_applied} patches written to audio.rs")
else:
    print("No changes made.")
