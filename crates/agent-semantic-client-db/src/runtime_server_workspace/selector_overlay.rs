//! Projection-kind validation for immutable selector generations.

pub(crate) fn validate_projection_kind(projection_kind: &str) -> Result<(), String> {
    match projection_kind {
        "source" | "callable-skeleton" => Ok(()),
        _ => Err(format!(
            "runtime selector projection kind is unsupported: projectionKind={projection_kind}"
        )),
    }
}
