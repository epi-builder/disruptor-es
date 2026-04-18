use es_projection::{
    CatchUpOutcome, MinimumGlobalPosition, ProjectionBatchLimit, ProjectionError, ProjectorName,
};

#[test]
fn minimum_position_rejects_empty_projector_name() {
    let error = ProjectorName::new("").expect_err("empty projector name must fail");

    assert_eq!(ProjectionError::InvalidProjectorName, error);
}

#[test]
fn minimum_position_rejects_negative_required_position() {
    let error =
        MinimumGlobalPosition::new(-1).expect_err("negative minimum position must fail");

    assert_eq!(ProjectionError::InvalidGlobalPosition { value: -1 }, error);
}

#[test]
fn minimum_position_rejects_invalid_batch_limits() {
    let zero = ProjectionBatchLimit::new(0).expect_err("zero batch limit must fail");
    let too_large =
        ProjectionBatchLimit::new(1001).expect_err("oversized batch limit must fail");

    assert_eq!(ProjectionError::InvalidBatchLimit { value: 0 }, zero);
    assert_eq!(
        ProjectionError::InvalidBatchLimit { value: 1001 },
        too_large
    );
}

#[test]
fn minimum_position_catch_up_outcome_exposes_applied_fields() {
    let outcome = CatchUpOutcome::Applied {
        event_count: 2,
        last_global_position: 7,
    };

    match outcome {
        CatchUpOutcome::Applied {
            event_count,
            last_global_position,
        } => {
            assert_eq!(2, event_count);
            assert_eq!(7, last_global_position);
        }
        CatchUpOutcome::Idle => panic!("expected applied outcome"),
    }
}
