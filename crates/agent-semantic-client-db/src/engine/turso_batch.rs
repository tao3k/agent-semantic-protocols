// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(super) const TURSO_BATCH_ROW_COUNT: usize = 64;

pub(super) fn text_value(value: &str) -> turso::Value {
    turso::Value::Text(value.to_owned())
}

pub(super) fn optional_text_value(value: Option<&str>) -> turso::Value {
    value.map_or(turso::Value::Null, text_value)
}

pub(super) fn append_parameter_rows(sql: &mut String, row_count: usize, column_count: usize) {
    for row_index in 0..row_count {
        if row_index > 0 {
            sql.push_str(", ");
        }
        sql.push('(');
        for column_index in 0..column_count {
            if column_index > 0 {
                sql.push_str(", ");
            }
            sql.push('?');
            sql.push_str(&(row_index * column_count + column_index + 1).to_string());
        }
        sql.push(')');
    }
}
