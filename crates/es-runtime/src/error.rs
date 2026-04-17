#[cfg(test)]
mod tests {
    use es_store_postgres::StoreError;

    use super::*;

    #[test]
    fn runtime_error_formats_typed_capacity_errors() {
        assert_eq!("runtime is overloaded", RuntimeError::Overloaded.to_string());
        assert_eq!("runtime is unavailable", RuntimeError::Unavailable.to_string());
        assert_eq!(
            "shard 2 is overloaded",
            RuntimeError::ShardOverloaded { shard_id: 2 }.to_string()
        );
    }

    #[test]
    fn runtime_error_maps_store_conflict_to_structured_conflict() {
        let error = RuntimeError::from_store_error(StoreError::StreamConflict {
            stream_id: "order-1".to_owned(),
            expected: "no stream".to_owned(),
            actual: Some(7),
        });

        assert!(matches!(
            error,
            RuntimeError::Conflict {
                stream_id,
                expected,
                actual: Some(7),
            } if stream_id == "order-1" && expected == "no stream"
        ));
    }
}
