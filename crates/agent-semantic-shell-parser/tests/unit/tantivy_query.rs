// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::analyze_tantivy_query;

#[test]
fn title_or_body_projects_only_the_positive_topology_feature_branch() {
    let analysis = analyze_tantivy_query("title:runtime^2 OR body:owner_snapshot");
    assert_eq!(analysis.selector_queries, ["owner_snapshot"]);
    assert!(analysis.selector_projection_complete);
}

#[test]
fn body_conjunction_remains_one_topology_posting_intersection() {
    let analysis = analyze_tantivy_query("title:runtime AND body:owner AND body:snapshot");
    assert_eq!(analysis.selector_queries, ["owner snapshot"]);
    assert!(analysis.selector_projection_complete);
}

#[test]
fn negative_or_structured_body_fails_closed_for_selector_projection() {
    let negative = analyze_tantivy_query("title:runtime AND -body:legacy");
    assert!(negative.selector_queries.is_empty());
    assert!(!negative.selector_projection_complete);

    let regex = analyze_tantivy_query("title:runtime OR body:/owner.*/");
    assert!(regex.selector_queries.is_empty());
    assert!(!regex.selector_projection_complete);
}
