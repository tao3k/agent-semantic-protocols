# Wendao Orgize Adapter Migration Plan

## Problem

Wendao client has useful `orgize` subcommands, but copying them directly into ASP would create a second hard-coded Org task language. That weakens `asp org search/query` because agents would learn command names instead of the reusable query axes underneath them.

The migration target is therefore adapter-first:

- expose Org facts through parser-owned `asp org search/query` fields
- publish guide recipes that compose those fields
- add compatibility subcommands only as thin wrappers over those recipes
- reserve destructive changes for an AST-patch or edit-plan contract
- keep `asp org` narrower than the standalone `orgize` debug CLI

## Command Inventory

Already native or close to native in standalone `languages/orgize` debug CLI:

- `eval plan`
- `eval patch`
- `sdd status`
- `sdd graph-diff`
- `agent-planning`
- `sparse-tree`
- `task-list`

Wendao read-model surfaces that must become query/search recipes before any compatibility shell:

- `read-model`
- `task-probe`
- `orgid-show`
- `task-sdd`
- `task-recover`
- `task-report`
- `task-archive`

## Query/Search Fact Work

Add missing Org document facts to the provider packet, not to ad hoc CLI branches:

- heading task state, priority, tags, effective tags, archive state, and planning timestamps
- property fields, including `ID`, `NEXT_ACTION`, `ARCHIVE_TARGET`, and `SDD_*`
- checklist progress and next unchecked item as derived parser facts
- section source selector for exact `orgid-show --full` replacement
- child heading frontier for compact task recovery
- task relation facts for SDD properties and parent links

These facts should be discoverable with normal selectors:

- `query --kind task --field todo=TODO`
- `query --kind checklistItem --field checked=true`
- `query --kind property --field key=ID --field value=<id>`
- `query --kind property --field key=NEXT_ACTION`
- `query --field tag=<tag>`
- `query --kind property --field key=SDD_KIND`

## Recipes

Guide recipes become the first migration surface:

- `task-probe`: property/tag/text query over task headings, capped and ranked
- `orgid-show`: ID property lookup plus section selector content
- `task-sdd`: ID lookup plus `SDD_*` property projection
- `task-recover`: active task query excluding done, archived, and closure-needed rows
- `task-report`: aggregate over task facts; no hidden DuckDB dependency
- `task-archive plan`: query-derived edit plan listing candidate source selectors and targets
- `task-archive apply`: separate AST-patch/edit-plan execution, never a search side effect

Compatibility commands may be added only after each recipe exists and prints the underlying `asp org query/search` replay command. They must not be exposed as top-level `asp org` document-provider commands unless the shared search/query contract explicitly accepts that command class.

## Safe Delete Gates

Wendao client `orgize` can be deleted only after these gates pass:

- command inventory test proves every Wendao subcommand maps to an ASP recipe or compatibility wrapper
- fixture parity tests cover read-model, task recovery, orgid lookup, SDD relation, report, and archive plan
- compatibility output includes the underlying query/search command so agents can graduate away from the wrapper
- no DuckDB, memory-engine, or Wendao-specific task index is copied into `languages/orgize`
- destructive archive apply is backed by AST-patch receipts and source selectors
- RFC and guide text state that recipes are conveniences over query/search facts, not a new domain grammar

## Implementation Order

1. Extend document query packet fields for task/property/checklist/archive facts.
2. Add `asp org guide` recipes for the semantic AST read-model use cases.
3. Add non-destructive recipe parity tests against Wendao fixtures.
4. Add compatibility wrappers that call the same query/search implementation and print replay commands.
5. Add archive plan as a query-derived edit plan.
6. Add archive apply through AST-patch after the edit-plan contract is reviewed.
7. Remove Wendao client `orgize` only after all gates are green.

## Current Landing Slice

The first correction slice lands only guide-level recipes with semantic names:

- `sdd-kind-properties` uses `asp org query --kind property --field key=SDD_KIND`
- `org-id-properties` uses `asp org query --kind property --field key=ID --field value=<ID>`
- `tagged-tasks` uses `asp org query --kind task --term <TEXT> --field tag=<TAG>`
- `done-tasks` uses `asp org query --kind task --field todo=DONE`
- `capture-task` uses `asp org capture-plan --kind task` plus caller-supplied properties

The Wendao lookup adapter should call these recipes and print the underlying
`asp org query/search` commands. It should not reintroduce legacy recipe labels
such as `wendao-task-*`, `wendao-orgid-*`, `sdd-property`, `agent-plan-*`, or
`plan-record` as provider-owned guide output.

`capture-plan` is the exception for recording a new plan because it is
non-mutating and renders a reviewable native Org entry plus application
preconditions. Its implementation lives in the Org semantic AST layer; the CLI
facade only forwards arguments and prints the rendered plan. Agent plan
contracts are ordinary caller-supplied Org properties and body content on a
generic capture kind such as `task`, not an `agent-plan` capture kind.

ASP document-provider command exposure is intentionally narrower than the
standalone `orgize` debug CLI. `sdd`, `agent-planning`, `sparse-tree`, and
`task-list` stay out of `asp org`; agents should use `asp org query` over
`kind=property`, `kind=task`, and `kind=checklistItem` instead.
