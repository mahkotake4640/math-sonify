#!/usr/bin/env python3
"""Patch main.rs to pass stereo_width to AudioEngine::start (idempotent)."""

with open('src/main.rs', 'r', encoding='utf-8') as f:
    c = f.read()

orig = c

# Fix import if needed
if 'StereoWidth' not in c:
    old = 'use crate::audio::{AudioEngine, WavRecorder, LoopExportPending, VuMeter, SidechainLevel, ClipBuffer, SnippetPlayback, SharedSnippetPlayback};'
    new = 'use crate::audio::{AudioEngine, WavRecorder, LoopExportPending, VuMeter, SidechainLevel, ClipBuffer, SnippetPlayback, SharedSnippetPlayback, StereoWidth};'
    if old in c:
        c = c.replace(old, new, 1)
        print("  applied import patch")
    else:
        print(f"  FAIL import - current: {repr(c[c.find('use crate::audio'):c.find('use crate::audio')+180])}")
else:
    print("  skip import (already has StereoWidth)")

# Create stereo_width_shared and pass to AudioEngine::start
if 'stereo_width_shared' not in c:
    # Find the snippet_pb line and AudioEngine::start call
    old = ('    let snippet_pb: SharedSnippetPlayback = Arc::new(Mutex::new(SnippetPlayback::idle()));\n'
           '\n'
           '    // Audio engine\n'
           '    let (_audio, actual_sr) = AudioEngine::start(')
    new = ('    let snippet_pb: SharedSnippetPlayback = Arc::new(Mutex::new(SnippetPlayback::idle()));\n'
           '\n'
           '    // #4 — Stereo width shared between UI and audio threads (default unity)\n'
           '    let stereo_width_shared: StereoWidth = Arc::new(Mutex::new(1.0f32));\n'
           '\n'
           '    // Audio engine\n'
           '    let (_audio, actual_sr) = AudioEngine::start(')
    if old in c:
        c = c.replace(old, new, 1)
        print("  applied stereo_width_shared creation")
    else:
        print("  FAIL stereo_width_shared creation")

    # Add stereo_width_shared.clone() as second-to-last arg before )?;
    old2 = ('        snippet_pb.clone(),\n'
            '    )?;')
    new2 = ('        snippet_pb.clone(),\n'
            '        stereo_width_shared.clone(),\n'
            '    )?;')
    if old2 in c:
        c = c.replace(old2, new2, 1)
        print("  applied arg pass")
    else:
        print("  FAIL arg pass - may have xrun_counter already")

    # Store in AppState
    old3 = ('        st.snippet_pb = snippet_pb;\n'
            '    }')
    new3 = ('        st.snippet_pb = snippet_pb;\n'
            '        // #4 — stereo width shared\n'
            '        st.stereo_width_shared = stereo_width_shared;\n'
            '    }')
    if old3 in c:
        c = c.replace(old3, new3, 1)
        print("  applied AppState store")
    else:
        print("  FAIL AppState store")
else:
    print("  skip main patches (stereo_width_shared already present)")

if c != orig:
    with open('src/main.rs', 'w', encoding='utf-8') as f:
        f.write(c)
    print("main.rs patched OK")
else:
    print("No changes to main.rs (already patched)")
