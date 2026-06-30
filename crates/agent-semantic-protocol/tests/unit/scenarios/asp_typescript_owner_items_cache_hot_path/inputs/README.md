# TypeScript owner-items cache hot path fixture

The scenario creates `app/src/model.ts`, activates a fake `ts-harness` with
owner-items capability, warms ASP once, and verifies the second request is
served from ASP's shared language owner-items cache.
