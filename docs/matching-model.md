# Matching Model Notes

## Baseline cost
`C_t(j) = alpha * target_distance + beta * transition_cost`

## Baseline terms
- `target_distance`: descriptor distance between current target frame and candidate grain
- `transition_cost`: weighted combination of descriptor continuity, normalized seek distance, and optional source-switch penalty

## Baseline defaults
- Distance metric: squared Euclidean on normalized descriptor vectors
- Selection: greedy per frame with previous-choice context
- Transition defaults:
  - descriptor continuity term enabled
  - normalized seek-distance term enabled
  - source-switch penalty enabled

## Extension points
- Reuse penalties
- Top-k stochastic selection
- Continuity constraints
- Path search variants

Keep extensions out of the first working matcher.
