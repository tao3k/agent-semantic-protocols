# Expected

After the overlay search adapter is warmed, repeated dirty-code lexical search
uses `source=turso-overlay` and does not spawn a provider process, call lexical, or
write a durable source snapshot.
