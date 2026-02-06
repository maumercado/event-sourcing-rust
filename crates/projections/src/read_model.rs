//! Read model trait for query-side views.

/// A read model providing query access to denormalized data.
///
/// Read models are the query-side data structures in CQRS.
/// They are updated by projections and optimized for fast reads.
pub trait ReadModel: Send + Sync {
    /// Returns the name of this read model.
    fn name(&self) -> &'static str;

    /// Returns the number of entries in this read model.
    fn count(&self) -> usize;
}
