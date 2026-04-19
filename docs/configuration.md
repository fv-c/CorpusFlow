# Configuration Reference

## CLI surface
- `cargo run -- show-config`: print the canonical default config as pretty JSON.
- `cargo run -- run [--config PATH] --output PATH`: load JSON config, run the offline pipeline, redraw a stage progress bar on `stderr` when attached to a terminal, fall back to line-based progress when redirected, and write the rendered WAV output.
- `cargo run -- validate-config PATH`: load JSON config and print the validated summary without running synthesis.

## Canonical default config
```json
{
  "corpus": {
    "root": "",
    "grain_size_ms": 100,
    "grain_hop_ms": 50,
    "mono_only": true
  },
  "target": {
    "path": "",
    "frame_size_ms": 100,
    "hop_size_ms": 50
  },
  "matching": {
    "alpha": 1.0,
    "beta": 0.25,
    "transition_descriptor_weight": 1.0,
    "transition_seek_weight": 0.5,
    "source_switch_penalty": 0.25
  },
  "micro_adaptation": {
    "gain": "off",
    "envelope": "off"
  },
  "synthesis": {
    "window": "hann",
    "output_hop_ms": 50,
    "overlap_schedule": "fixed",
    "irregularity_ms": 0
  },
  "rendering": {
    "output_sample_rate": 48000,
    "mode": "mono",
    "stereo_routing": "duplicate-mono",
    "post_convolution": {
      "enabled": false,
      "source": "target",
      "audio_path": "",
      "dry_mix": 1.0,
      "wet_mix": 1.0,
      "normalize_output": true
    },
    "ambisonics": {
      "positioning_json_path": ""
    }
  }
}
```

## Section notes
- `corpus`: fixed-grid grain planning for corpus ingestion. `root` is the WAV file or directory root. `mono_only=true` keeps the mono corpus baseline explicit by downmixing stereo corpus WAVs before segmentation.
- `target`: target analysis input and frame grid. `path` is the target WAV path.
- `matching`: baseline target and transition cost weights. All values must be finite.
- `micro_adaptation`: deterministic post-selection gain and carrier-envelope modes. Allowed values are `off`, `match-target-rms`, and `inherit-carrier-rms`. Carrier-envelope transfer follows the scheduled synthesis timeline, so it stays aligned even when `target.hop_size_ms` and `synthesis.output_hop_ms` differ.
- `synthesis`: overlap-add windowing and scheduling. Current `window` baseline is `hann`.
- `rendering`: output sample rate, output routing, and optional post-convolution. Corpus and target inputs are resampled to `output_sample_rate` before segmentation, analysis, and synthesis. When post-convolution is enabled, the convolution audio comes either from the original target file (`source = "target"`) or from an explicit WAV path (`source = "audio-file"` with `audio_path`). Ambisonics stays reserved behind explicit JSON positioning input.

## Validation rules
- Unknown JSON fields are rejected at every config level.
- `corpus.grain_size_ms > 0`
- `corpus.grain_hop_ms > 0`
- `target.frame_size_ms > 0`
- `target.hop_size_ms > 0`
- matching weights must all be finite
- `synthesis.output_hop_ms > 0`
- `rendering.output_sample_rate > 0`
- `overlap_schedule = "fixed"` requires `irregularity_ms = 0`
- `overlap_schedule = "alternating"` requires `irregularity_ms > 0` and `< output_hop_ms`
- `post_convolution.dry_mix` and `wet_mix` must stay within `0.0..=1.0`
- enabled `post_convolution` with `source = "audio-file"` requires a non-empty `audio_path`
- `rendering.mode = "ambisonics-reserved"` requires a readable positioning JSON with a non-empty strictly increasing trajectory starting at `time_ms = 0`

## Run-time requirements
- `run` requires a non-empty `corpus.root`.
- `run` requires a non-empty `target.path`.
- `run` requires an explicit `--output PATH`.
- `run` creates the parent directory for the output WAV when it does not already exist.
- enabled `post_convolution` with `source = "target"` reuses the original target WAV as convolution audio and resamples it to `output_sample_rate`
- enabled `post_convolution` with `source = "audio-file"` requires a readable WAV at `audio_path`; mono and stereo files are accepted, wider channel layouts are averaged to mono before convolution

## Enum values
- `micro_adaptation.gain`: `off`, `match-target-rms`
- `micro_adaptation.envelope`: `off`, `inherit-carrier-rms`
- `synthesis.window`: `hann`
- `synthesis.overlap_schedule`: `fixed`, `alternating`
- `rendering.mode`: `mono`, `stereo`, `ambisonics-reserved`
- `rendering.stereo_routing`: `duplicate-mono`
- `rendering.post_convolution.source`: `target`, `audio-file`
