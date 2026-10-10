-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SemanticProjectionRuntimeWrapping

namespace ASPProof.Audit.SemanticProjectionRuntimeWrapping

open ASPProof.SemanticProjectionRuntimeWrapping

#check provider_payload_cannot_supply_envelope_authority
#check verified_binding_preserves_runtime_authority
#check payload_digest_mismatch_rejected
#check root_digest_does_not_replace_generation_binding

end ASPProof.Audit.SemanticProjectionRuntimeWrapping
