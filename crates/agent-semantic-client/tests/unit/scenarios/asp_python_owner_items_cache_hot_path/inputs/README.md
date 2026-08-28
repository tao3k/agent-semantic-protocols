# Python owner-items cache hot path fixture

The scenario creates `src/model.py`, activates a fake `py-harness` with
owner-items capability, warms ASP once, and verifies the second request is
served from ASP's shared language owner-items cache.
