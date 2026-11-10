/// Join every Tokio workspace task and restore deterministic plan order.
///
/// The caller submits every independent workspace task before awaiting this
/// boundary. Tokio owns runtime scheduling; this function adds no leaf
/// semaphore or fixed concurrency limit.
pub async fn join_tasks_in_plan_order<T: Send + 'static>(
    mut tasks: tokio::task::JoinSet<Result<(usize, T), String>>,
    expected_count: usize,
) -> Result<Vec<(usize, T)>, String> {
    let mut completed = Vec::with_capacity(expected_count);
    while let Some(result) = tasks.join_next().await {
        completed.push(result.map_err(|error| format!("ASP workspace task failed: {error}"))??);
    }
    if completed.len() != expected_count {
        return Err(format!(
            "ASP workspace scheduler did not execute every selected task: expected={expected_count} actual={}",
            completed.len()
        ));
    }
    completed.sort_by_key(|(plan_index, _)| *plan_index);
    Ok(completed)
}

#[cfg(test)]
#[path = "../tests/unit/plan_order.rs"]
mod tests;
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
