//! Storage-neutral outbox dispatcher.

use es_core::TenantId;
use futures::future::BoxFuture;
use metrics::{counter, gauge};
use time::OffsetDateTime;
use tracing::{Instrument, info_span};
use uuid::Uuid;

use crate::{
    DispatchBatchLimit, DispatchOutcome, OutboxMessage, OutboxResult, Publisher, RetryPolicy,
    RetryScheduleOutcome, WorkerId,
};

/// Storage boundary used by the outbox dispatcher.
pub trait OutboxStore: Clone + Send + Sync + 'static {
    /// Claims due pending rows for a dispatcher worker.
    fn claim_pending(
        &self,
        tenant_id: TenantId,
        worker_id: WorkerId,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<OutboxMessage>>>;

    /// Marks a row as published after the publisher completes successfully.
    fn mark_published(
        &self,
        tenant_id: TenantId,
        outbox_id: Uuid,
        worker_id: WorkerId,
    ) -> BoxFuture<'_, OutboxResult<()>>;

    /// Schedules another retry attempt or transitions a row to failed.
    fn schedule_retry(
        &self,
        tenant_id: TenantId,
        outbox_id: Uuid,
        worker_id: WorkerId,
        error: String,
        retry_policy: RetryPolicy,
    ) -> BoxFuture<'_, OutboxResult<RetryScheduleOutcome>>;
}

/// Claims and dispatches one bounded batch of outbox rows.
pub async fn dispatch_once<S, P>(
    store: &S,
    publisher: &P,
    tenant_id: TenantId,
    worker_id: WorkerId,
    limit: DispatchBatchLimit,
    retry_policy: RetryPolicy,
) -> OutboxResult<DispatchOutcome>
where
    S: OutboxStore,
    P: Publisher,
{
    let claimed = store
        .claim_pending(tenant_id.clone(), worker_id.clone(), limit)
        .await?;
    if claimed.is_empty() {
        counter!("es_outbox_dispatch_total", "outcome" => "idle").increment(1);
        return Ok(DispatchOutcome::Idle);
    }

    let mut published = 0;
    let mut retried = 0;
    let mut failed = 0;

    for message in claimed {
        let topic = message.topic.as_str().to_owned();
        let lag_seconds = (OffsetDateTime::now_utc() - message.created_at)
            .as_seconds_f64()
            .max(0.0);
        gauge!("es_outbox_lag", "topic" => topic.clone()).set(lag_seconds);
        let span = info_span!(
            "outbox.dispatch",
            tenant_id = %tenant_id.as_str(),
            topic = %topic,
            global_position = message.source.global_position(),
        );
        let publish_result = publisher
            .publish(message.publish_envelope())
            .instrument(span)
            .await;
        match publish_result {
            Ok(()) => {
                store
                    .mark_published(tenant_id.clone(), message.outbox_id, worker_id.clone())
                    .await?;
                counter!("es_outbox_dispatch_total", "outcome" => "published").increment(1);
                published += 1;
            }
            Err(error) => {
                match store
                    .schedule_retry(
                        tenant_id.clone(),
                        message.outbox_id,
                        worker_id.clone(),
                        error.to_string(),
                        retry_policy,
                    )
                    .await?
                {
                    RetryScheduleOutcome::RetryScheduled => {
                        counter!("es_outbox_dispatch_total", "outcome" => "retried").increment(1);
                        retried += 1;
                    }
                    RetryScheduleOutcome::Failed => {
                        counter!("es_outbox_dispatch_total", "outcome" => "failed").increment(1);
                        failed += 1;
                    }
                }
            }
        }
    }

    if retried == 0 && failed == 0 {
        Ok(DispatchOutcome::Published { published })
    } else {
        Ok(DispatchOutcome::Partial {
            published,
            retried,
            failed,
        })
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use std::sync::{Arc, Mutex};

    use es_core::TenantId;
    use futures::future::BoxFuture;
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;
    use crate::{
        InMemoryPublisher, MessageKey, OutboxStatus, SourceEventRef, Topic, dispatch_once,
    };

    #[derive(Clone, Debug)]
    struct FakeOutboxStore {
        inner: Arc<Mutex<FakeOutboxStoreInner>>,
    }

    #[derive(Debug)]
    struct FakeOutboxStoreInner {
        claimed: Vec<OutboxMessage>,
        marked: Vec<Uuid>,
        retries: Vec<(Uuid, String, RetryPolicy)>,
        retry_outcome: RetryScheduleOutcome,
    }

    impl FakeOutboxStore {
        fn new(claimed: Vec<OutboxMessage>) -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeOutboxStoreInner {
                    claimed,
                    marked: Vec::new(),
                    retries: Vec::new(),
                    retry_outcome: RetryScheduleOutcome::RetryScheduled,
                })),
            }
        }

        fn with_retry_outcome(self, retry_outcome: RetryScheduleOutcome) -> Self {
            self.inner.lock().expect("fake store mutex").retry_outcome = retry_outcome;
            self
        }

        fn marked(&self) -> Vec<Uuid> {
            self.inner.lock().expect("fake store mutex").marked.clone()
        }

        fn retries(&self) -> Vec<(Uuid, String, RetryPolicy)> {
            self.inner.lock().expect("fake store mutex").retries.clone()
        }
    }

    impl OutboxStore for FakeOutboxStore {
        fn claim_pending(
            &self,
            _tenant_id: TenantId,
            _worker_id: WorkerId,
            _limit: DispatchBatchLimit,
        ) -> BoxFuture<'_, OutboxResult<Vec<OutboxMessage>>> {
            Box::pin(
                async move { Ok(self.inner.lock().expect("fake store mutex").claimed.clone()) },
            )
        }

        fn mark_published(
            &self,
            _tenant_id: TenantId,
            outbox_id: Uuid,
            _worker_id: WorkerId,
        ) -> BoxFuture<'_, OutboxResult<()>> {
            Box::pin(async move {
                self.inner
                    .lock()
                    .expect("fake store mutex")
                    .marked
                    .push(outbox_id);
                Ok(())
            })
        }

        fn schedule_retry(
            &self,
            _tenant_id: TenantId,
            outbox_id: Uuid,
            _worker_id: WorkerId,
            error: String,
            retry_policy: RetryPolicy,
        ) -> BoxFuture<'_, OutboxResult<RetryScheduleOutcome>> {
            Box::pin(async move {
                let mut inner = self.inner.lock().expect("fake store mutex");
                inner.retries.push((outbox_id, error, retry_policy));
                Ok(inner.retry_outcome)
            })
        }
    }

    fn tenant_id() -> TenantId {
        TenantId::new("tenant-a").expect("valid tenant id")
    }

    fn worker_id() -> WorkerId {
        WorkerId::new("worker-a").expect("valid worker id")
    }

    fn batch_limit() -> DispatchBatchLimit {
        DispatchBatchLimit::new(10).expect("valid batch limit")
    }

    fn retry_policy() -> RetryPolicy {
        RetryPolicy::new(2).expect("valid retry policy")
    }

    fn message() -> OutboxMessage {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid timestamp");
        OutboxMessage {
            outbox_id: Uuid::from_u128(1),
            tenant_id: tenant_id(),
            source: SourceEventRef::new(Uuid::from_u128(2), 10).expect("valid source event ref"),
            topic: Topic::new("orders.placed").expect("valid topic"),
            message_key: MessageKey::new("order-1").expect("valid message key"),
            payload: json!({ "order_id": "order-1" }),
            metadata: json!({ "kind": "integration" }),
            status: OutboxStatus::Publishing,
            attempts: 1,
            available_at: now,
            locked_by: Some(worker_id()),
            locked_until: Some(now),
            published_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn dispatcher_returns_idle_when_no_rows_are_claimed() -> OutboxResult<()> {
        futures::executor::block_on(async {
            let store = FakeOutboxStore::new(Vec::new());
            let publisher = InMemoryPublisher::default();

            let outcome = dispatch_once(
                &store,
                &publisher,
                tenant_id(),
                worker_id(),
                batch_limit(),
                retry_policy(),
            )
            .await?;

            assert_eq!(DispatchOutcome::Idle, outcome);
            assert!(publisher.published().is_empty());
            assert!(store.marked().is_empty());
            Ok(())
        })
    }

    #[test]
    fn dispatcher_marks_success_after_publish() -> OutboxResult<()> {
        futures::executor::block_on(async {
            let row = message();
            let store = FakeOutboxStore::new(vec![row.clone()]);
            let publisher = InMemoryPublisher::default();

            let outcome = dispatch_once(
                &store,
                &publisher,
                tenant_id(),
                worker_id(),
                batch_limit(),
                retry_policy(),
            )
            .await?;

            assert_eq!(DispatchOutcome::Published { published: 1 }, outcome);
            assert_eq!(vec![row.outbox_id], store.marked());
            assert_eq!(1, publisher.published().len());
            Ok(())
        })
    }

    #[test]
    fn dispatcher_schedules_retry_after_publish_failure() -> OutboxResult<()> {
        futures::executor::block_on(async {
            let row = message();
            let store = FakeOutboxStore::new(vec![row.clone()]);
            let publisher = InMemoryPublisher::default();
            publisher.push_failure("broker down");

            let outcome = dispatch_once(
                &store,
                &publisher,
                tenant_id(),
                worker_id(),
                batch_limit(),
                retry_policy(),
            )
            .await?;

            assert_eq!(
                DispatchOutcome::Partial {
                    published: 0,
                    retried: 1,
                    failed: 0
                },
                outcome
            );
            assert!(store.marked().is_empty());
            assert_eq!(
                vec![(
                    row.outbox_id,
                    "publisher error: broker down".to_owned(),
                    retry_policy()
                )],
                store.retries()
            );
            Ok(())
        })
    }

    #[test]
    fn dispatcher_reports_failed_when_retry_policy_is_exhausted() -> OutboxResult<()> {
        futures::executor::block_on(async {
            let row = message();
            let store =
                FakeOutboxStore::new(vec![row]).with_retry_outcome(RetryScheduleOutcome::Failed);
            let publisher = InMemoryPublisher::default();
            publisher.push_failure("broker down");

            let outcome = dispatch_once(
                &store,
                &publisher,
                tenant_id(),
                worker_id(),
                batch_limit(),
                retry_policy(),
            )
            .await?;

            assert_eq!(
                DispatchOutcome::Partial {
                    published: 0,
                    retried: 0,
                    failed: 1
                },
                outcome
            );
            assert!(store.marked().is_empty());
            Ok(())
        })
    }

    #[test]
    fn dispatcher_preserves_idempotency_key() -> OutboxResult<()> {
        futures::executor::block_on(async {
            let row = message();
            let expected_key = row.idempotency_key();
            let store = FakeOutboxStore::new(vec![row.clone(), row]);
            let publisher = InMemoryPublisher::default();

            let outcome = dispatch_once(
                &store,
                &publisher,
                tenant_id(),
                worker_id(),
                batch_limit(),
                retry_policy(),
            )
            .await?;

            assert_eq!(DispatchOutcome::Published { published: 2 }, outcome);
            let published = publisher.published();
            assert_eq!(1, published.len());
            assert_eq!(expected_key, published[0].idempotency_key);
            assert_eq!("order-1", published[0].message_key);
            Ok(())
        })
    }
}
