pub(crate) fn retained_row_limit(column_count: usize, max_retained_cells: Option<usize>) -> usize {
    max_retained_cells
        .map(|cell_limit| {
            let column_count = column_count.max(1);
            (cell_limit / column_count).max(1)
        })
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_row_limit_caps_wide_query_results_by_cells() {
        assert_eq!(retained_row_limit(200, Some(300_000)), 1_500);
        assert_eq!(retained_row_limit(20, Some(300_000)), 15_000);
        assert_eq!(retained_row_limit(0, Some(300_000)), 300_000);
        assert_eq!(retained_row_limit(200, None), usize::MAX);
        assert_eq!(retained_row_limit(200, Some(0)), 1);
    }
}
