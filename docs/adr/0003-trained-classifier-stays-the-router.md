# 3. The trained classifier stays the router

**Decision.** Free wording is routed by the trained network (pruned multilingual MiniLM,
fine-tuned on Iris' acts). Skills declare which acts mean which intent. Skills also carry
training templates per language; the network is retrained from them, behind a regression gate.

**Measured (2026-11-13, held-out sets in `nlu/data`).**

| Router | Right act |
|---|---|
| Trained network (build 115) | about 95 % |
| Untrained encoder, nearest of 25 500 example sentences | 85.6 % |
| Untrained encoder, 10 examples per act | 50 % |
| Untrained encoder, 200 examples per act | 82 % |
| Frozen old network, new acts from 10 / 50 examples | 28 % / 57 % |
| Ari's keyword + example-phrase tiers (re-implemented), English rows Ari has a skill for | 53 %, 45 % unrouted |

The test sentences were written for Iris and favour it; Ari's tiers were re-implemented, not
run, and its cloud tier was not measured.

**Why.** "A skill brings example phrases and is understood" does not hold with this encoder.

**Consequences.** A new skill is understood through its patterns at once and through free
wording only after a retrain. Retraining must be routine: templates in the skill, one command,
and a gate that refuses a network that got worse on old sentences.
