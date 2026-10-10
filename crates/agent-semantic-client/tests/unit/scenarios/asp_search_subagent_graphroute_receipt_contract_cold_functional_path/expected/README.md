# ASP Search Subagent Org/GQL Contract

The scenario fixes the managed read-only `asp-explore` stop contract:

- successful output is exactly one Org source block with language `gql`
- the block carries `:profile search-evidence.v1 :eval never`
- malformed flat receipts and multiple blocks are rejected
- source bodies, snippets, line-range selectors, confidence labels, and not-found inventories are rejected
- the recovery action asks the same child session to re-emit the sole Org/GQL format without Example/Grammar text

This scenario keeps subagent Search output identical to the root Search presentation contract.
