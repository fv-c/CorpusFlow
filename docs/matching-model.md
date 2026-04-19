# Matching Model Notes

## Baseline cost
`C_t(j) = alpha * target_distance + beta * transition_cost`

## Baseline terms
- `target_distance`: descriptor distance between current target frame and candidate grain
- `transition_cost`: descriptor distance between previous selected grain and current candidate

## Baseline defaults
- Distance metric: squared Euclidean on normalized descriptor vectors
- Selection: greedy per frame with previous-choice context

## Extension points
- Reuse penalties
- Top-k stochastic selection
- Continuity constraints
- Path search variants

Keep extensions out of the first working matcher.
