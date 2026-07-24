use serde_json::Value;

use super::{
    RENDERED_SEARCH_PROJECTION_SCHEMA_ID, RenderedSearchProjectionV1,
    SEARCH_PROJECTION_SCHEMA_VERSION, SearchProjectionDensityV1, SearchProjectionError,
    SearchProjectionRequestV1,
    topology::{self, TopologyProjectionOptions},
};

pub trait SearchProjectionRenderer {
    fn render(
        &self,
        packet: &dyn crate::source::SearchProjectionSource,
        request: &SearchProjectionRequestV1,
    ) -> Result<RenderedSearchProjectionV1, SearchProjectionError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TopologySearchProjectionRenderer;

impl SearchProjectionRenderer for TopologySearchProjectionRenderer {
    fn render(
        &self,
        packet: &dyn crate::source::SearchProjectionSource,
        request: &SearchProjectionRequestV1,
    ) -> Result<RenderedSearchProjectionV1, SearchProjectionError> {
        request.validate()?;
        if request.projection_id != "topology" {
            return Err(SearchProjectionError::InvalidRequest(format!(
                "topology renderer does not support projectionId={}",
                request.projection_id
            )));
        }
        let content = topology::render_search_topology_projection(
            packet.as_value(),
            TopologyProjectionOptions {
                density: request.density,
                seed_limit: request.max_rows,
            },
        );
        if let Some(max_bytes) = request.max_bytes {
            if content.len() > max_bytes {
                return Err(SearchProjectionError::BudgetExceeded {
                    actual_bytes: content.len(),
                    max_bytes,
                });
            }
        }
        Ok(RenderedSearchProjectionV1 {
            schema_id: RENDERED_SEARCH_PROJECTION_SCHEMA_ID.to_string().into(),
            schema_version: SEARCH_PROJECTION_SCHEMA_VERSION.to_string().into(),
            projection_id: request.projection_id.clone(),
            density: request.density,
            semantic_digest: packet.semantic_digest().to_string(),
            content_type: "text/plain; charset=utf-8".to_string(),
            content,
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RankedFrontierSearchProjectionRenderer;

impl SearchProjectionRenderer for RankedFrontierSearchProjectionRenderer {
    fn render(
        &self,
        packet: &dyn crate::source::SearchProjectionSource,
        request: &SearchProjectionRequestV1,
    ) -> Result<RenderedSearchProjectionV1, SearchProjectionError> {
        request.validate()?;
        if request.projection_id != "ranked-frontier" {
            return Err(SearchProjectionError::InvalidRequest(format!(
                "ranked-frontier renderer does not support projectionId={}",
                request.projection_id
            )));
        }
        let value = packet.as_value();
        if value.get("schemaId").and_then(serde_json::Value::as_str)
            != Some(crate::source::SEMANTIC_GRAPH_TURBO_RESULT_SCHEMA_ID)
        {
            return Err(SearchProjectionError::InvalidPacket(
                "ranked-frontier projection requires a semantic graph-turbo result".to_string(),
            ));
        }
        let ranked_nodes = value
            .get("rankedNodes")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                SearchProjectionError::InvalidPacket(
                    "graph-turbo result rankedNodes must be an array".to_string(),
                )
            })?;
        let row_limit = request.max_rows.unwrap_or(ranked_nodes.len());
        let profile = scalar(value.get("profile"));
        let algorithm = scalar(value.get("algorithm"));
        let mut lines = vec![format!(
            "[search-frontier] projection=ranked-frontier density={} profile={} algorithm={} nodes={}",
            density_name(request.density),
            profile,
            algorithm,
            ranked_nodes.len().min(row_limit)
        )];
        let scores = value.get("scores").and_then(serde_json::Value::as_object);
        for node in ranked_nodes.iter().take(row_limit) {
            let id = scalar(node.get("id"));
            let kind = scalar(node.get("kind"));
            let action = scalar(node.get("action"));
            let node_value = scalar(node.get("value"));
            let mut line = format!("I={id} kind={kind} action={action} value={node_value}");
            if !matches!(request.density, SearchProjectionDensityV1::Terse) {
                let role = scalar(node.get("role"));
                let score = scores
                    .and_then(|scores| scores.get(&id))
                    .and_then(serde_json::Value::as_f64)
                    .map(|score| format!("{score:.6}"))
                    .unwrap_or_else(|| "-".to_string());
                line.push_str(&format!(" role={role} score={score}"));
            }
            lines.push(line);
        }
        if matches!(request.density, SearchProjectionDensityV1::Expanded) {
            if let Some(paths) = value
                .get("typedPaths")
                .and_then(serde_json::Value::as_array)
            {
                for path in paths.iter().take(row_limit) {
                    lines.push(format!(
                        "P={} source={} sink={} kind={}",
                        scalar(path.get("id")),
                        scalar(path.get("source")),
                        scalar(path.get("sink")),
                        scalar(path.get("pathKind"))
                    ));
                }
            }
        }
        let content = format!("{}\n", lines.join("\n"));
        if let Some(max_bytes) = request.max_bytes {
            if content.len() > max_bytes {
                return Err(SearchProjectionError::BudgetExceeded {
                    actual_bytes: content.len(),
                    max_bytes,
                });
            }
        }
        Ok(RenderedSearchProjectionV1 {
            schema_id: RENDERED_SEARCH_PROJECTION_SCHEMA_ID.to_string().into(),
            schema_version: SEARCH_PROJECTION_SCHEMA_VERSION.to_string().into(),
            projection_id: request.projection_id.clone(),
            density: request.density,
            semantic_digest: packet.semantic_digest().to_string(),
            content_type: "text/plain; charset=utf-8".to_string(),
            content,
        })
    }
}

fn density_name(density: SearchProjectionDensityV1) -> &'static str {
    match density {
        SearchProjectionDensityV1::Terse => "terse",
        SearchProjectionDensityV1::Standard => "standard",
        SearchProjectionDensityV1::Expanded => "expanded",
    }
}

fn scalar(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(serde_json::Value::as_str)
        .unwrap_or("-")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn render_search_topology_projection(
    packet: &Value,
    options: TopologyProjectionOptions,
) -> String {
    topology::render_search_topology_projection(packet, options)
}
