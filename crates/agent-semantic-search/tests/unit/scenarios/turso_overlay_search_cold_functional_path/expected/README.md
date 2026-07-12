# Expected

The cold dynamic overlay path writes one transient overlay document through the
Turso DB Engine search adapter and reads it back with `source=turso-overlay`.
It must not invoke a provider process, fall back to lexical, or persist a durable
source snapshot just to perform lexical search.
