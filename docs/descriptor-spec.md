# Descriptor Specification

## Recommended baseline
Compact per-frame vector for ~100 ms grains:

1. Log RMS
2. Zero-crossing rate
3. Spectral centroid
4. Spectral flatness
5. Spectral rolloff (85%)

## Why this baseline
- Low computational cost
- Reasonable timbral discrimination
- Small dimensionality for fast search and matching
- Extensible later without changing the stage boundary

## One short alternative
- Replace items 3 to 5 with coarse log band energies if stronger spectral shape control is needed.

Recommendation: keep the five-feature baseline first.
