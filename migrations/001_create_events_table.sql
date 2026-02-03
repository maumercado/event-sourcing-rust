-- Event store schema
-- Stores all domain events in an append-only fashion

-- Enable pgcrypto for gen_random_uuid() if not available (PG < 13)
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(255) NOT NULL,
    aggregate_id UUID NOT NULL,
    aggregate_type VARCHAR(255) NOT NULL,
    version BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',

    -- Optimistic concurrency: unique version per aggregate
    CONSTRAINT unique_aggregate_version UNIQUE (aggregate_id, version)
);

-- Indexes for common queries
CREATE INDEX idx_events_aggregate_id ON events(aggregate_id);
CREATE INDEX idx_events_event_type ON events(event_type);
CREATE INDEX idx_events_timestamp ON events(timestamp);

-- Snapshot table for aggregate state caching
CREATE TABLE snapshots (
    aggregate_id UUID PRIMARY KEY,
    aggregate_type VARCHAR(255) NOT NULL,
    version BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    state JSONB NOT NULL
);
