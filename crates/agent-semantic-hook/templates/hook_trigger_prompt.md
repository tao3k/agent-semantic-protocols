<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-BEGIN -->
ASP hook blocked `{reason}`; do not retry raw read/search commands on the same source.
Codex: use one resident ASP search agent for this main task. If an ASP search agent thread is already open, call `send_input` on that recorded agent id with the safe route below and wait for `asp-search-subagent(role,action,evidence,missing,next,risk)`. Do not create another search agent for another `actionFrontier` item in the same main task.
If no ASP search agent is open, call `spawn_agent` once with `agent_type="asp_explorer"` when the project role is available, otherwise fall back to `agent_type="explorer"`; pass `fork_context=false` and a self-contained ASP Explorer branch prompt. Keep model and reasoning settings in Codex config. Record the returned `agent-...` id in the parent reasoning ledger and reuse it with `send_input` for later ASP searches.
If subagents are unavailable, run the safe route directly.

{routes}
<!-- ASP-HOOK-TRIGGER-PROMPT:MANAGED-END -->

<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-BEGIN -->
<!-- Add project-local hook trigger guidance below. `asp install hook` preserves this block. -->
<!-- ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-END -->
