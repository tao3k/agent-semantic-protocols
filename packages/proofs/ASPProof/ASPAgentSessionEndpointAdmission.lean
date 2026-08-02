namespace ASPProof.AgentSessionEndpointAdmission

/-- The host may publish a Runtime Server endpoint using a regular file,
    a Unix-domain socket, or another transport-owned filesystem node. -/
structure EndpointObservation where
  present : Bool
  regularFile : Bool
  deriving DecidableEq, Repr

def unixSocketEndpoint : EndpointObservation :=
  { present := true, regularFile := false }

/-- The rejected implementation confused publication with regular-file type. -/
def legacyPublished (endpoint : EndpointObservation) : Bool :=
  endpoint.regularFile

/-- Admission depends on endpoint publication, independently of node type. -/
def published (endpoint : EndpointObservation) : Bool :=
  endpoint.present

inductive RegistryRoute where
  | runtimeServerProxy
  | directTurso
  deriving DecidableEq, Repr

def route (endpoint : EndpointObservation) : RegistryRoute :=
  if published endpoint then
    .runtimeServerProxy
  else
    .directTurso

theorem unixSocket_is_counterexample_to_regular_file_detection :
    published unixSocketEndpoint = true ∧
      legacyPublished unixSocketEndpoint = false := by
  decide

theorem published_endpoint_routes_through_runtime_server
    (endpoint : EndpointObservation)
    (hPublished : published endpoint = true) :
    route endpoint = .runtimeServerProxy := by
  simp [route, hPublished]

theorem published_endpoint_forbids_direct_turso
    (endpoint : EndpointObservation)
    (hPublished : published endpoint = true) :
    route endpoint ≠ .directTurso := by
  simp [route, hPublished]

theorem endpoint_node_type_cannot_authorize_direct_open
    (endpoint : EndpointObservation)
    (hPublished : published endpoint = true) :
    endpoint.regularFile = false → route endpoint = .runtimeServerProxy := by
  intro _
  exact published_endpoint_routes_through_runtime_server endpoint hPublished

end ASPProof.AgentSessionEndpointAdmission
