#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_result_alias_accepts_store_error() {
        let result: StoreResult<()> = Err(StoreError::EmptyAppend);

        assert!(matches!(result, Err(StoreError::EmptyAppend)));
    }
}
