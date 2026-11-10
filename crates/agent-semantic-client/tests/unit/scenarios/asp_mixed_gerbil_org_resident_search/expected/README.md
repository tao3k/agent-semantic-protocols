# Expected

The first call may perform bounded candidate parser materialization for Gerbil.
The repeated call must:

- return the same generation-bound GQL result;
- report Runtime dispatch below `1000us`;
- perform no generation build or generation wait;
- start no provider for Org; and
- require no installed `asp-org` receipt or artifact.

CLI startup, IPC setup, and output rendering are reported separately and are
not represented as Runtime search-kernel time.
