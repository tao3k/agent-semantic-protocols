# ASP Search Subagent GraphRoute Receipt Contract

The scenario fixes the managed read-only `asp-explore` stop contract:

- valid receipts include `schema`, `intent`, `route`, `state`, ranked selector evidence, and exactly one safe parent `next` action
- malformed flat receipts with `owner/read/next` are rejected
- source bodies, snippets, line-range selectors, confidence labels, and not-found inventories are rejected
- the recovery action asks the same child session to re-emit a compact graph-route receipt

This scenario protects the parent model from noisy subagent transcripts and keeps the search path aligned with `ReasoningTree -> EvidenceGraph -> GraphRoute -> exact parent action`.
