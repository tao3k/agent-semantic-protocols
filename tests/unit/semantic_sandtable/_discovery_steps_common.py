"""Discovery, schema-loading, capture, and stdin behavior tests."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.semantic_sandtable.runner import discover_scenarios, run_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]

__all__ = [
    "Path",
    "discover_scenarios",
    "json",
    "os",
    "patch",
    "run_scenario",
    "subprocess",
    "sys",
    "tempfile",
    "unittest",
]
