-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.CodexThreadManagementAuthority

#print axioms ASPProof.CodexThreadManagementAuthority.deeplink_is_one_thread_identity
#print axioms ASPProof.CodexThreadManagementAuthority.navigation_does_not_communicate
#print axioms ASPProof.CodexThreadManagementAuthority.read_and_wait_do_not_start_cross_thread_work
#print axioms ASPProof.CodexThreadManagementAuthority.send_message_is_the_only_communication_operation
#print axioms ASPProof.CodexThreadManagementAuthority.observation_and_communication_are_disjoint
