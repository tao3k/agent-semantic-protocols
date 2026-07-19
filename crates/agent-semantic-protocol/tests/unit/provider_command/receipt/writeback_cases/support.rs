pub(crate) const PRIME_DECISION_LINE: &str = "|decision purpose=decision-primer answer=false code=false capabilities=lexical,pipe,fd-query,rg-query,owner-items,selector-code,treesitter-query ladder=lexical>fd-query|rg-query>owner-items>selector-code>pipe history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath risk=broad-direct-read,manual-window-scan,repeat-prime next=\"asp rust search lexical --query '<question-term>' --query '<related-feature-term>' owner tests --workspace . --view seeds\"";

pub(crate) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
