# CorpusFlow

CorpusFlow e' una CLI offline in Rust per corpus-based granular matching e audio resynthesis.

Lo stato attuale del progetto copre la pipeline baseline end-to-end:

1. ingestione corpus WAV mono
2. segmentazione a griglia fissa
3. estrazione e normalizzazione dei descrittori
4. analisi del target
5. matching greedy con costo locale e di transizione
6. sintesi granulare overlap-add
7. rendering mono/stereo e scrittura WAV

## Stato implementazione

La baseline attuale e' pensata per essere semplice, deterministica e ispezionabile.

- Corpus mono-first: il corpus viene caricato come WAV mono.
- Target mono/stereo: il target puo' essere stereo, ma viene analizzato in mono tramite mixdown.
- Matching: baseline greedy con combinazione di target cost e transition cost.
- Synthesis: overlap-add con finestra Hann e scheduling `fixed` o `alternating`.
- Micro-adaptation: supporto a gain matching per-grain e carrier-envelope shaping globale.
- Rendering: output `mono` o `stereo` con routing `duplicate-mono`.
- Post-process: convoluzione opzionale con `dry/wet mix` e normalizzazione.
- Ambisonics: interfaccia di configurazione presente, rendering audio non ancora implementato.

Architetturalmente il progetto mantiene gli stage separati in moduli distinti:

- [src/corpus.rs](/Users/master/Documents/GitHub/CorpusFlow/src/corpus.rs)
- [src/index.rs](/Users/master/Documents/GitHub/CorpusFlow/src/index.rs)
- [src/target.rs](/Users/master/Documents/GitHub/CorpusFlow/src/target.rs)
- [src/matching.rs](/Users/master/Documents/GitHub/CorpusFlow/src/matching.rs)
- [src/synthesis.rs](/Users/master/Documents/GitHub/CorpusFlow/src/synthesis.rs)
- [src/rendering.rs](/Users/master/Documents/GitHub/CorpusFlow/src/rendering.rs)
- [src/app.rs](/Users/master/Documents/GitHub/CorpusFlow/src/app.rs)

Per i dettagli architetturali e di configurazione:

- [docs/architecture.md](/Users/master/Documents/GitHub/CorpusFlow/docs/architecture.md)
- [docs/configuration.md](/Users/master/Documents/GitHub/CorpusFlow/docs/configuration.md)

## Requisiti

- Rust stabile
- Cargo

Il progetto usa dipendenze intenzionalmente ristrette:

- `hound` per I/O WAV
- `rustfft` per descrittori spettrali
- `serde` e `serde_json` per configurazione esplicita
- `criterion` per benchmark locali

## Installazione

Clonare il repository e compilare in release:

```bash
git clone <repo-url>
cd CorpusFlow
cargo build --release
```

Il build produce il binario CLI in `target/release/corpusflow`.

Per usare direttamente il comando `corpusflow` dalla shell, installare il binario localmente via Cargo:

```bash
cargo install --path .
```

Per sviluppo locale:

```bash
cargo test
```

## Utilizzo rapido

Stampare la configurazione canonica di default:

```bash
corpusflow show-config
```

Validare un file JSON di configurazione:

```bash
corpusflow validate-config config.json
```

Eseguire la pipeline end-to-end e scrivere il WAV finale:

```bash
corpusflow run --config config.json --output out/render.wav
```

Se il path di output contiene directory non ancora esistenti, CorpusFlow le crea automaticamente.

## Configurazione

La CLI usa un file JSON esplicito. I campi principali sono:

- `corpus`: root del corpus WAV, dimensione grain, hop, modalita' mono con downmix stereo prima della segmentazione
- `target`: path del target WAV e griglia di analisi
- `matching`: pesi del modello di costo
- `micro_adaptation`: gain ed envelope post-selezione
- `synthesis`: finestra e scheduling overlap-add
- `rendering`: modalita' di uscita, sample rate, convoluzione opzionale, hook ambisonics

Esempio minimo:

```json
{
  "corpus": {
    "root": "examples/corpus",
    "grain_size_ms": 100,
    "grain_hop_ms": 50,
    "mono_only": true
  },
  "target": {
    "path": "examples/target.wav",
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

Se `rendering.mode = "ambisonics-reserved"`, `rendering.ambisonics.positioning_json_path` deve puntare a un JSON esterno con traiettoria del centro spaziale e jitter separato della nuvola, per esempio:

```json
{
  "space": "cartesian",
  "loop": false,
  "default_curve": "linear",
  "trajectory": [
    {
      "time_ms": 0,
      "position": { "x": 0.0, "y": 1.0, "z": 0.0 },
      "to_next": { "curve": "linear" }
    },
    {
      "time_ms": 1200,
      "position": { "x": 0.6, "y": 0.2, "z": 0.1 },
      "to_next": { "curve": "catmull-rom", "tension": 0.5 }
    },
    {
      "time_ms": 2600,
      "position": { "x": -0.3, "y": 0.4, "z": 0.2 }
    }
  ],
  "jitter": {
    "mode": "gaussian",
    "per_grain": true,
    "seed": 42,
    "spread": { "x": 0.08, "y": 0.08, "z": 0.04 },
    "smoothing_ms": 80
  }
}
```

Per l'elenco completo di regole, enum e vincoli:

- [docs/configuration.md](/Users/master/Documents/GitHub/CorpusFlow/docs/configuration.md)

## Esempio workflow

1. Preparare una cartella di WAV mono per il corpus.
2. Preparare un file WAV target.
3. Esportare il default config con `corpusflow show-config > config.json`.
4. Aggiornare `corpus.root` e `target.path`.
5. Eseguire `corpusflow run --config config.json --output out/render.wav`.

## Benchmark e validazione

Test completi:

```bash
cargo test
```

Benchmark disponibili:

```bash
cargo bench --bench descriptor_extraction
cargo bench --bench candidate_scoring
cargo bench --bench greedy_matching
cargo bench --bench overlap_add_synthesis
cargo bench --bench offline_render_pipeline
```

Riferimenti:

- [docs/release-validation.md](/Users/master/Documents/GitHub/CorpusFlow/docs/release-validation.md)
- [docs/release-notes.md](/Users/master/Documents/GitHub/CorpusFlow/docs/release-notes.md)

## Limiti attuali

- Il corpus baseline richiede WAV mono.
- Ambisonics e' solo riservato a un'estensione futura.
- La validazione della config controlla struttura e valori, ma non forza a priori l'esistenza dei path fino al comando `run`.
- Le metriche di benchmark sono significative solo se confrontate sulla stessa macchina/toolchain.

## Layout repository

```text
src/
  app.rs
  audio.rs
  cli.rs
  config.rs
  corpus.rs
  descriptor.rs
  index.rs
  matching.rs
  micro_adaptation.rs
  rendering.rs
  synthesis.rs
  target.rs
docs/
benches/
tests/
```
