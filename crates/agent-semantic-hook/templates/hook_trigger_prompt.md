<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-BEGIN -->
ASP hook blocked `{reason}`; do not retry raw read/search commands on the same source.
{agent_flow}
Search recovery is evidence-state driven. If the prompt, prior packet, failure transcript, or hook route already has an exact selector, owner, symbol, dependency, or `recommendedNext`, start there and do not run `prime` first. Use `prime` or `ingest` routes only when the workspace topology or owner map is unknown. Use `pipe` only when a previous frontier exposes ambiguity that needs query-set refinement. If a route has no hits, return a compact `noOutput` or no-candidates receipt instead of expanding an empty search.

{routes}
<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-END -->

<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-BEGIN -->
<!-- Add project-local hook trigger guidance below. `asp install hook` preserves this block. -->
<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-END -->
