"""Canonical frozen-entry boundary for the Graph Turbo resident service.

The release bundle is an executable projection of the same package-owned
resident CLI used in development.  Keeping this boundary in the package makes
the frozen artifact part of the provider contract rather than a build-support
wrapper with an independent import path.
"""

from asp_graph_turbo.resident_cli import main


if __name__ == "__main__":
    main()
