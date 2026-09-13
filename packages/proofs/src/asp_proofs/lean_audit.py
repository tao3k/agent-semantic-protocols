# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Public Lean audit admission API."""

from .lean_admission import AuditAdmissionError, admit_audit

__all__ = ["AuditAdmissionError", "admit_audit"]
