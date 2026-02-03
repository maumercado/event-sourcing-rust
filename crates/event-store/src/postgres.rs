use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

use crate::{
    AggregateId, EventEnvelope, EventId, EventQuery, EventStoreError, Result, Snapshot, Version,
    store::{AppendOptions, EventStore, EventStream, validate_events_for_append},
};

/// PostgreSQL-backed event store implementation.
#[derive(Clone)]
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    /// Creates a new PostgreSQL event store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Gets a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Runs the database migrations.
    pub async fn run_migrations(&self) -> std::result::Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("../../migrations").run(&self.pool).await
    }

    fn row_to_event(row: PgRow) -> Result<EventEnvelope> {
        let metadata_json: serde_json::Value = row.try_get("metadata")?;
        let metadata: HashMap<String, serde_json::Value> = serde_json::from_value(metadata_json)?;

        Ok(EventEnvelope {
            event_id: EventId::from_uuid(row.try_get::<Uuid, _>("id")?),
            event_type: row.try_get("event_type")?,
            aggregate_id: AggregateId::from_uuid(row.try_get::<Uuid, _>("aggregate_id")?),
            aggregate_type: row.try_get("aggregate_type")?,
            version: Version::new(row.try_get("version")?),
            timestamp: row.try_get("timestamp")?,
            payload: row.try_get("payload")?,
            metadata,
        })
    }
}

#[async_trait]
impl EventStore for PostgresEventStore {
    async fn append(&self, events: Vec<EventEnvelope>, options: AppendOptions) -> Result<Version> {
        validate_events_for_append(&events).map_err(|e| {
            EventStoreError::Serialization(serde_json::Error::io(std::io::Error::other(e.message)))
        })?;

        let first_event = &events[0];
        let aggregate_id = first_event.aggregate_id;

        // Start a transaction
        let mut tx = self.pool.begin().await?;

        // Check expected version if specified
        if let Some(expected) = options.expected_version {
            let current_version: Option<i64> =
                sqlx::query_scalar("SELECT MAX(version) FROM events WHERE aggregate_id = $1")
                    .bind(aggregate_id.as_uuid())
                    .fetch_one(&mut *tx)
                    .await?;

            let actual = Version::new(current_version.unwrap_or(0));

            if actual != expected {
                return Err(EventStoreError::ConcurrencyConflict {
                    aggregate_id,
                    expected,
                    actual,
                });
            }
        }

        // Insert all events
        let mut last_version = Version::initial();
        for event in &events {
            let metadata_json = serde_json::to_value(&event.metadata)?;

            sqlx::query(
                r#"
                INSERT INTO events (id, event_type, aggregate_id, aggregate_type, version, timestamp, payload, metadata)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(event.event_id.as_uuid())
            .bind(&event.event_type)
            .bind(event.aggregate_id.as_uuid())
            .bind(&event.aggregate_type)
            .bind(event.version.as_i64())
            .bind(event.timestamp)
            .bind(&event.payload)
            .bind(metadata_json)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                // Check if this is a unique constraint violation (concurrency conflict)
                if let sqlx::Error::Database(ref db_err) = e
                    && db_err.constraint() == Some("unique_aggregate_version")
                {
                    return EventStoreError::ConcurrencyConflict {
                        aggregate_id,
                        expected: options.expected_version.unwrap_or(Version::initial()),
                        actual: event.version,
                    };
                }
                EventStoreError::Database(e)
            })?;

            last_version = event.version;
        }

        tx.commit().await?;
        Ok(last_version)
    }

    async fn get_events_for_aggregate(
        &self,
        aggregate_id: AggregateId,
    ) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_type, aggregate_id, aggregate_type, version, timestamp, payload, metadata
            FROM events
            WHERE aggregate_id = $1
            ORDER BY version ASC
            "#,
        )
        .bind(aggregate_id.as_uuid())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn get_events_for_aggregate_from_version(
        &self,
        aggregate_id: AggregateId,
        from_version: Version,
    ) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_type, aggregate_id, aggregate_type, version, timestamp, payload, metadata
            FROM events
            WHERE aggregate_id = $1 AND version >= $2
            ORDER BY version ASC
            "#,
        )
        .bind(aggregate_id.as_uuid())
        .bind(from_version.as_i64())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn query_events(&self, query: EventQuery) -> Result<Vec<EventEnvelope>> {
        let mut sql = String::from(
            "SELECT id, event_type, aggregate_id, aggregate_type, version, timestamp, payload, metadata FROM events WHERE 1=1",
        );
        let mut param_count = 0;

        // Build dynamic query
        if query.aggregate_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND aggregate_id = ${param_count}"));
        }
        if query.aggregate_type.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND aggregate_type = ${param_count}"));
        }
        if query.event_types.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND event_type = ANY(${param_count})"));
        }
        if query.from_version.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND version >= ${param_count}"));
        }
        if query.to_version.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND version <= ${param_count}"));
        }
        if query.from_timestamp.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND timestamp >= ${param_count}"));
        }
        if query.to_timestamp.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND timestamp <= ${param_count}"));
        }

        sql.push_str(" ORDER BY timestamp ASC, version ASC");

        if query.limit.is_some() {
            param_count += 1;
            sql.push_str(&format!(" LIMIT ${param_count}"));
        }
        if query.offset.is_some() {
            param_count += 1;
            sql.push_str(&format!(" OFFSET ${param_count}"));
        }

        // Build and execute query with parameters
        let mut sqlx_query = sqlx::query(&sql);

        if let Some(id) = query.aggregate_id {
            sqlx_query = sqlx_query.bind(id.as_uuid());
        }
        if let Some(agg_type) = query.aggregate_type {
            sqlx_query = sqlx_query.bind(agg_type);
        }
        if let Some(event_types) = query.event_types {
            sqlx_query = sqlx_query.bind(event_types);
        }
        if let Some(from_version) = query.from_version {
            sqlx_query = sqlx_query.bind(from_version.as_i64());
        }
        if let Some(to_version) = query.to_version {
            sqlx_query = sqlx_query.bind(to_version.as_i64());
        }
        if let Some(from_ts) = query.from_timestamp {
            sqlx_query = sqlx_query.bind(from_ts);
        }
        if let Some(to_ts) = query.to_timestamp {
            sqlx_query = sqlx_query.bind(to_ts);
        }
        if let Some(limit) = query.limit {
            sqlx_query = sqlx_query.bind(limit as i64);
        }
        if let Some(offset) = query.offset {
            sqlx_query = sqlx_query.bind(offset as i64);
        }

        let rows = sqlx_query.fetch_all(&self.pool).await?;
        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn get_events_by_type(&self, event_type: &str) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_type, aggregate_id, aggregate_type, version, timestamp, payload, metadata
            FROM events
            WHERE event_type = $1
            ORDER BY timestamp ASC
            "#,
        )
        .bind(event_type)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn stream_all_events(&self) -> Result<EventStream> {
        use futures_util::StreamExt;

        let stream = sqlx::query(
            r#"
            SELECT id, event_type, aggregate_id, aggregate_type, version, timestamp, payload, metadata
            FROM events
            ORDER BY timestamp ASC, id ASC
            "#,
        )
        .fetch(&self.pool)
        .map(|result| match result {
            Ok(row) => Self::row_to_event(row),
            Err(e) => Err(EventStoreError::Database(e)),
        });

        Ok(Box::pin(stream))
    }

    async fn get_aggregate_version(&self, aggregate_id: AggregateId) -> Result<Option<Version>> {
        let version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM events WHERE aggregate_id = $1")
                .bind(aggregate_id.as_uuid())
                .fetch_one(&self.pool)
                .await?;

        Ok(version.map(Version::new))
    }

    async fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO snapshots (aggregate_id, aggregate_type, version, timestamp, state)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (aggregate_id) DO UPDATE SET
                aggregate_type = EXCLUDED.aggregate_type,
                version = EXCLUDED.version,
                timestamp = EXCLUDED.timestamp,
                state = EXCLUDED.state
            "#,
        )
        .bind(snapshot.aggregate_id.as_uuid())
        .bind(&snapshot.aggregate_type)
        .bind(snapshot.version.as_i64())
        .bind(snapshot.timestamp)
        .bind(&snapshot.state)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_snapshot(&self, aggregate_id: AggregateId) -> Result<Option<Snapshot>> {
        let row: Option<PgRow> = sqlx::query(
            r#"
            SELECT aggregate_id, aggregate_type, version, timestamp, state
            FROM snapshots
            WHERE aggregate_id = $1
            "#,
        )
        .bind(aggregate_id.as_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Snapshot {
                aggregate_id: AggregateId::from_uuid(row.try_get::<Uuid, _>("aggregate_id")?),
                aggregate_type: row.try_get("aggregate_type")?,
                version: Version::new(row.try_get("version")?),
                timestamp: row.try_get::<DateTime<Utc>, _>("timestamp")?,
                state: row.try_get("state")?,
            })),
            None => Ok(None),
        }
    }
}
