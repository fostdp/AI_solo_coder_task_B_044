use axum::{
    extract::{Query, State},
    Json,
};

use maritime_common::models::*;

use crate::AppState;

pub async fn not_found_handler() -> &'static str {
    "Not Found"
}

pub fn validate_year_range(year_start: Option<i32>, year_end: Option<i32>) -> (i32, i32) {
    let start = year_start.unwrap_or(-1000);
    let end = year_end.unwrap_or(1800);
    if start > end {
        (end, start)
    } else {
        (start, end)
    }
}

pub fn default_if_empty<T: Default>(val: Option<T>) -> T {
    val.unwrap_or_default()
}
