-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.RustHarnessPackageAtomicity

namespace ASPProof.Audit.RustHarnessPackageAtomicity

open ASPProof.RustHarnessPackageAtomicity

#check package_scope_admits_only_its_cargo_owner
#check missing_root_manifest_admits_no_owner
#check sibling_change_is_not_admitted_by_package_scope
#check explicit_workspace_scope_admits_selected_member
#check lexical_prefix_does_not_imply_same_cargo_package
#check wrapper_relabeling_cannot_change_package_admission

end ASPProof.Audit.RustHarnessPackageAtomicity
