// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Lossless conversion between the shared protocol model and its canonical
//! Schema Manager-owned protobuf wire projection.

use agent_semantic_client_protocol::ClientCapabilities;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientInfo;
use agent_semantic_client_protocol::ClientMethod;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientParameter;
use agent_semantic_client_protocol::ClientParameterCardinality;
use agent_semantic_client_protocol::ClientParameterSource;
use agent_semantic_client_protocol::ClientParameterType;
use agent_semantic_client_protocol::ClientProtocolCatalog;
use agent_semantic_client_protocol::ClientTransport;
use agent_semantic_client_protocol::TraceContext;
use serde_json::Value;

use super::generated as wire;

pub(super) fn encode_frame(frame: ClientFrame) -> Result<wire::ClientFrameEnvelope, String> {
    use wire::client_frame_envelope::Frame;

    let (base, frame) = match frame {
        ClientFrame::Initialize {
            base,
            request_id,
            client_info,
            capabilities,
        } => (
            base,
            Frame::Initialize(wire::InitializeFrame {
                request_id: request_id.into_inner(),
                client_info: Some(encode_client_info(client_info)),
                capabilities_json: encode_json(&capabilities)?,
            }),
        ),
        ClientFrame::Request {
            base,
            request_id,
            catalog_generation,
            workspace_generation,
            method,
            params,
        } => (
            base,
            Frame::Request(wire::RequestFrame {
                request_id: request_id.into_inner(),
                catalog_generation,
                workspace_generation,
                method,
                params_json: encode_json(&params)?,
            }),
        ),
        ClientFrame::Cancel { base, request_id } => (
            base,
            Frame::Cancel(wire::CancelFrame {
                request_id: request_id.into_inner(),
            }),
        ),
        ClientFrame::Shutdown { base, request_id } => (
            base,
            Frame::Shutdown(wire::ShutdownFrame {
                request_id: request_id.into_inner(),
            }),
        ),
        ClientFrame::Exit { base } => (base, Frame::Exit(wire::ExitFrame {})),
        ClientFrame::Response {
            base,
            request_id,
            outcome,
            result,
            error,
            catalog,
        } => (
            base,
            Frame::Response(wire::ResponseFrame {
                request_id: request_id.into_inner(),
                outcome: encode_outcome(outcome),
                result_json: encode_optional_json(result.as_ref())?,
                error_json: encode_optional_json(error.as_ref())?,
                catalog: catalog.map(encode_catalog),
            }),
        ),
        ClientFrame::Event {
            base,
            event_id,
            event,
            payload,
        } => (
            base,
            Frame::Event(wire::EventFrame {
                event_id,
                event,
                payload_json: encode_json(&payload)?,
            }),
        ),
    };
    Ok(wire::ClientFrameEnvelope {
        base: Some(encode_base(base)),
        frame: Some(frame),
    })
}

pub(super) fn decode_frame(envelope: wire::ClientFrameEnvelope) -> Result<ClientFrame, String> {
    use wire::client_frame_envelope::Frame;

    let base = decode_base(require(envelope.base, "base")?)?;
    match require(envelope.frame, "frame")? {
        Frame::Initialize(frame) => Ok(ClientFrame::Initialize {
            base,
            request_id: identifier(frame.request_id, "requestId")?,
            client_info: decode_client_info(require(frame.client_info, "clientInfo")?)?,
            capabilities: decode_json(&frame.capabilities_json, "capabilities")?,
        }),
        Frame::Request(frame) => Ok(ClientFrame::Request {
            base,
            request_id: identifier(frame.request_id, "requestId")?,
            catalog_generation: required(frame.catalog_generation, "catalogGeneration")?,
            workspace_generation: required(frame.workspace_generation, "workspaceGeneration")?,
            method: required(frame.method, "method")?,
            params: decode_json(&frame.params_json, "params")?,
        }),
        Frame::Cancel(frame) => Ok(ClientFrame::Cancel {
            base,
            request_id: identifier(frame.request_id, "requestId")?,
        }),
        Frame::Shutdown(frame) => Ok(ClientFrame::Shutdown {
            base,
            request_id: identifier(frame.request_id, "requestId")?,
        }),
        Frame::Exit(_) => Ok(ClientFrame::Exit { base }),
        Frame::Response(frame) => Ok(ClientFrame::Response {
            base,
            request_id: identifier(frame.request_id, "requestId")?,
            outcome: decode_outcome(frame.outcome)?,
            result: decode_optional_json(&frame.result_json, "result")?,
            error: decode_optional_json(&frame.error_json, "error")?,
            catalog: frame.catalog.map(decode_catalog).transpose()?,
        }),
        Frame::Event(frame) => Ok(ClientFrame::Event {
            base,
            event_id: required(frame.event_id, "eventId")?,
            event: required(frame.event, "event")?,
            payload: decode_json(&frame.payload_json, "payload")?,
        }),
        Frame::ResponsePartition(_) => {
            Err("response partitions must be reassembled by the gRPC transport".to_owned())
        }
    }
}

fn encode_base(base: ClientFrameBase) -> wire::ClientFrameBase {
    wire::ClientFrameBase {
        schema_id: base.schema_id,
        schema_version: base.schema_version,
        protocol_id: base.protocol_id,
        protocol_version: base.protocol_version,
        session_id: base.session_id.into_inner(),
        project_id: base.project_id.into_inner(),
        workspace_id: base.workspace_id.into_inner(),
        trace_context: base.trace_context.map(|trace| wire::TraceContext {
            traceparent: trace.traceparent,
            tracestate: trace.tracestate,
        }),
    }
}

fn decode_base(base: wire::ClientFrameBase) -> Result<ClientFrameBase, String> {
    Ok(ClientFrameBase {
        schema_id: required(base.schema_id, "schemaId")?,
        schema_version: required(base.schema_version, "schemaVersion")?,
        protocol_id: required(base.protocol_id, "protocolId")?,
        protocol_version: required(base.protocol_version, "protocolVersion")?,
        session_id: identifier(base.session_id, "sessionId")?,
        project_id: identifier(base.project_id, "projectId")?,
        workspace_id: identifier(base.workspace_id, "workspaceId")?,
        trace_context: base
            .trace_context
            .map(|trace| -> Result<TraceContext, String> {
                Ok(TraceContext {
                    traceparent: required(trace.traceparent, "traceparent")?,
                    tracestate: trace.tracestate,
                })
            })
            .transpose()?,
    })
}

fn encode_client_info(info: ClientInfo) -> wire::ClientInfo {
    wire::ClientInfo {
        name: info.name,
        version: info.version,
    }
}

fn decode_client_info(info: wire::ClientInfo) -> Result<ClientInfo, String> {
    Ok(ClientInfo {
        name: required(info.name, "clientInfo.name")?,
        version: required(info.version, "clientInfo.version")?,
    })
}

fn encode_catalog(catalog: ClientProtocolCatalog) -> wire::ClientProtocolCatalog {
    wire::ClientProtocolCatalog {
        schema_id: catalog.schema_id,
        schema_version: catalog.schema_version,
        protocol_id: catalog.protocol_id,
        protocol_version: catalog.protocol_version,
        catalog_generation: catalog.catalog_generation,
        workspace_generation: catalog.workspace_generation,
        transports: catalog
            .transports
            .into_iter()
            .map(|transport| match transport {
                ClientTransport::RuntimeIpc => wire::ClientTransport::RuntimeIpc as i32,
            })
            .collect(),
        capabilities: Some(wire::ClientCapabilities {
            request_cancellation: catalog.capabilities.request_cancellation,
            events: catalog.capabilities.events,
            streaming: catalog.capabilities.streaming,
            trace_context: catalog.capabilities.trace_context,
        }),
        methods: catalog.methods.into_iter().map(encode_method).collect(),
    }
}

fn decode_catalog(catalog: wire::ClientProtocolCatalog) -> Result<ClientProtocolCatalog, String> {
    let capabilities = require(catalog.capabilities, "catalog.capabilities")?;
    Ok(ClientProtocolCatalog {
        schema_id: required(catalog.schema_id, "catalog.schemaId")?,
        schema_version: required(catalog.schema_version, "catalog.schemaVersion")?,
        protocol_id: required(catalog.protocol_id, "catalog.protocolId")?,
        protocol_version: required(catalog.protocol_version, "catalog.protocolVersion")?,
        catalog_generation: required(catalog.catalog_generation, "catalog.catalogGeneration")?,
        workspace_generation: required(
            catalog.workspace_generation,
            "catalog.workspaceGeneration",
        )?,
        transports: catalog
            .transports
            .into_iter()
            .map(
                |transport| match wire::ClientTransport::try_from(transport) {
                    Ok(wire::ClientTransport::RuntimeIpc) => Ok(ClientTransport::RuntimeIpc),
                    _ => Err("unsupported client transport on protobuf wire".to_owned()),
                },
            )
            .collect::<Result<_, _>>()?,
        capabilities: ClientCapabilities {
            request_cancellation: capabilities.request_cancellation,
            events: capabilities.events,
            streaming: capabilities.streaming,
            trace_context: capabilities.trace_context,
        },
        methods: catalog
            .methods
            .into_iter()
            .map(decode_method)
            .collect::<Result<_, _>>()?,
    })
}

fn encode_method(method: ClientMethod) -> wire::ClientMethod {
    wire::ClientMethod {
        method: method.method,
        route_id: method.route_id.into_inner(),
        request_schema_id: method.request_schema_id.into_inner(),
        response_schema_id: method.response_schema_id.into_inner(),
        error_schema_ids: method
            .error_schema_ids
            .into_iter()
            .map(agent_semantic_client_protocol::ClientSchemaId::into_inner)
            .collect(),
        parameters: method
            .parameters
            .into_iter()
            .map(encode_parameter)
            .collect(),
        cancellable: method.cancellable,
        streaming: method.streaming,
    }
}

fn decode_method(method: wire::ClientMethod) -> Result<ClientMethod, String> {
    Ok(ClientMethod {
        method: required(method.method, "catalog.method.method")?,
        route_id: agent_semantic_client_protocol::ClientRouteId::new(required(
            method.route_id,
            "catalog.method.routeId",
        )?)?,
        request_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(required(
            method.request_schema_id,
            "catalog.method.requestSchemaId",
        )?)?,
        response_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(required(
            method.response_schema_id,
            "catalog.method.responseSchemaId",
        )?)?,
        error_schema_ids: method
            .error_schema_ids
            .into_iter()
            .map(agent_semantic_client_protocol::ClientSchemaId::new)
            .collect::<Result<_, _>>()?,
        parameters: method
            .parameters
            .into_iter()
            .map(decode_parameter)
            .collect::<Result<_, _>>()?,
        cancellable: method.cancellable,
        streaming: method.streaming,
    })
}

fn encode_parameter(parameter: ClientParameter) -> wire::ClientParameter {
    wire::ClientParameter {
        name: parameter.name,
        value_type: match parameter.value_type {
            ClientParameterType::String => wire::ClientParameterType::String as i32,
            ClientParameterType::StringArray => wire::ClientParameterType::StringArray as i32,
            ClientParameterType::WorkspaceRelativePath => {
                wire::ClientParameterType::WorkspaceRelativePath as i32
            }
            ClientParameterType::StructuralSelector => {
                wire::ClientParameterType::StructuralSelector as i32
            }
            ClientParameterType::Presentation => wire::ClientParameterType::Presentation as i32,
            ClientParameterType::Boolean => wire::ClientParameterType::Boolean as i32,
            ClientParameterType::UnsignedInteger => {
                wire::ClientParameterType::UnsignedInteger as i32
            }
            ClientParameterType::Json => wire::ClientParameterType::Json as i32,
        },
        cardinality: match parameter.cardinality {
            ClientParameterCardinality::Required => {
                wire::ClientParameterCardinality::Required as i32
            }
            ClientParameterCardinality::Optional => {
                wire::ClientParameterCardinality::Optional as i32
            }
            ClientParameterCardinality::Many => wire::ClientParameterCardinality::Many as i32,
        },
        source: match parameter.source {
            ClientParameterSource::Request => wire::ClientParameterSource::Request as i32,
            ClientParameterSource::RuntimeContext => {
                wire::ClientParameterSource::RuntimeContext as i32
            }
        },
    }
}

fn decode_parameter(parameter: wire::ClientParameter) -> Result<ClientParameter, String> {
    Ok(ClientParameter {
        name: required(parameter.name, "catalog.parameter.name")?,
        value_type: match wire::ClientParameterType::try_from(parameter.value_type) {
            Ok(wire::ClientParameterType::String) => ClientParameterType::String,
            Ok(wire::ClientParameterType::StringArray) => ClientParameterType::StringArray,
            Ok(wire::ClientParameterType::WorkspaceRelativePath) => {
                ClientParameterType::WorkspaceRelativePath
            }
            Ok(wire::ClientParameterType::StructuralSelector) => {
                ClientParameterType::StructuralSelector
            }
            Ok(wire::ClientParameterType::Presentation) => ClientParameterType::Presentation,
            Ok(wire::ClientParameterType::Boolean) => ClientParameterType::Boolean,
            Ok(wire::ClientParameterType::UnsignedInteger) => ClientParameterType::UnsignedInteger,
            Ok(wire::ClientParameterType::Json) => ClientParameterType::Json,
            _ => return Err("unsupported client parameter type on protobuf wire".to_owned()),
        },
        cardinality: match wire::ClientParameterCardinality::try_from(parameter.cardinality) {
            Ok(wire::ClientParameterCardinality::Required) => ClientParameterCardinality::Required,
            Ok(wire::ClientParameterCardinality::Optional) => ClientParameterCardinality::Optional,
            Ok(wire::ClientParameterCardinality::Many) => ClientParameterCardinality::Many,
            _ => {
                return Err("unsupported client parameter cardinality on protobuf wire".to_owned());
            }
        },
        source: match wire::ClientParameterSource::try_from(parameter.source) {
            Ok(wire::ClientParameterSource::Request) => ClientParameterSource::Request,
            Ok(wire::ClientParameterSource::RuntimeContext) => {
                ClientParameterSource::RuntimeContext
            }
            _ => return Err("unsupported client parameter source on protobuf wire".to_owned()),
        },
    })
}

fn encode_outcome(outcome: ClientOutcome) -> i32 {
    match outcome {
        ClientOutcome::Ready => wire::ClientOutcome::Ready as i32,
        ClientOutcome::Error => wire::ClientOutcome::Error as i32,
        ClientOutcome::Cancelled => wire::ClientOutcome::Cancelled as i32,
        ClientOutcome::StaleGeneration => wire::ClientOutcome::StaleGeneration as i32,
    }
}

fn decode_outcome(outcome: i32) -> Result<ClientOutcome, String> {
    match wire::ClientOutcome::try_from(outcome) {
        Ok(wire::ClientOutcome::Ready) => Ok(ClientOutcome::Ready),
        Ok(wire::ClientOutcome::Error) => Ok(ClientOutcome::Error),
        Ok(wire::ClientOutcome::Cancelled) => Ok(ClientOutcome::Cancelled),
        Ok(wire::ClientOutcome::StaleGeneration) => Ok(ClientOutcome::StaleGeneration),
        _ => Err("unsupported client outcome on protobuf wire".to_owned()),
    }
}

fn encode_json(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|error| format!("encode dynamic JSON payload: {error}"))
}

fn encode_optional_json(value: Option<&Value>) -> Result<Vec<u8>, String> {
    value
        .map(encode_json)
        .transpose()
        .map(Option::unwrap_or_default)
}

fn decode_json(bytes: &[u8], field: &str) -> Result<Value, String> {
    if bytes.is_empty() {
        return Err(format!("protobuf ClientFrame {field} must be present"));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| format!("decode protobuf ClientFrame {field}: {error}"))
}

fn decode_optional_json(bytes: &[u8], field: &str) -> Result<Option<Value>, String> {
    if bytes.is_empty() {
        Ok(None)
    } else {
        decode_json(bytes, field).map(Some)
    }
}

fn require<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("protobuf ClientFrame {field} must be present"))
}

fn required(value: String, field: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err(format!("protobuf ClientFrame {field} must be non-empty"))
    } else {
        Ok(value)
    }
}

fn identifier<T>(value: String, field: &str) -> Result<T, String>
where
    T: TryFrom<String, Error = String>,
{
    T::try_from(value).map_err(|error| format!("invalid protobuf ClientFrame {field}: {error}"))
}

#[cfg(test)]
#[path = "../../tests/unit/grpc_wire.rs"]
mod tests;
