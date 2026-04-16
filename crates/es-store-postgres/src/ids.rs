/// Generates storage-layer identifiers in Rust.
pub trait IdGenerator {
    /// Returns a new event identifier.
    fn new_event_id(&self) -> uuid::Uuid;
}

/// UUIDv7 event identifier generator.
#[derive(Clone, Copy, Debug, Default)]
pub struct UuidV7Generator;

impl IdGenerator for UuidV7Generator {
    fn new_event_id(&self) -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }
}

#[cfg(test)]
mod ids {
    use super::*;

    #[test]
    fn uuid_v7_generator_returns_version_7_uuid() {
        let event_id = UuidV7Generator.new_event_id();

        assert_eq!(Some(uuid::Version::SortRand), event_id.get_version());
    }
}
