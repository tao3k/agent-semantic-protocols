import ASPProof.Audit.SearchRouteTemporalCapabilityLease
import ASPProof.Audit.Receipt

open ASPProof.Audit

run_cmd do
  writeReceipt
    "receipts/searchroute-temporal-capability-lease-audit-v1.json"
    SearchRouteTemporalCapabilityLease.auditJson
