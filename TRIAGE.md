# Triage

Stage 7. One row per request after the person has used the build: a bug, a change, or a new idea, in the person's own words. Rows are numbered T1, T2 and so on, and each commit that answers one names it ("Fix the timer reset (T4)"). `C4 Verify against hand-computed cases`.

Compare at least one result with a case worked out by hand. **Severity** is *blocks use*, *wrong*, or *wanted*. **Cost** is the agent's estimate. **Disposition** is *done*, *open*, *declined* (with the reason), or *try* (built on a branch and published as a TRY commit that says how to undo it).

| # | Request, in the person's words | Expected | What happened | Severity | Cost | Disposition |
| --- | --- | --- | --- | --- | --- | --- |
| T1 | "the game should be over the shoulder third person" (from CW-Q4) | The camera sits behind and above the right shoulder in travel and in duels; the sheathed mouse-look turns that camera rather than switching to another view. | Default had a free look when sheathed. | wanted | small | open |
| T2 | "you don't start with deflect, you have to learn it from a trainer, who makes you practice it with them before you can do it yourself" (from CW-Q11) | A trainer in the MVP region teaches the imbue; a practice round against the trainer's slow pattern must be passed before Q works anywhere else. | Default had the imbue known from the start. | wanted | medium | open |
| T3 | "there should be tests should ensure that there is absolutely no flickering" | Tests fail on z-fighting, on detail-level popping and on frame-to-frame shimmer. | | wanted | medium | open |
| T4 | "water in the game should use a solver like extremeley life like fishing" | River and pond water move with the fishing game's solver. | | wanted | medium | open |
| T5 | "publish to a public github repo, and a public github pages for the deployment" | Live at sgilson7.github.io/chumworld/ after each milestone. | | wanted | small | open |
