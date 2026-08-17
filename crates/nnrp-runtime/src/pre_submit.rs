use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use nnrp_core::SchedulingMetadata;

use crate::RuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reservation {
    frame_id: u32,
    metadata: SchedulingMetadata,
}

#[derive(Debug, Default)]
pub(crate) struct PreSubmitDeadlineReservations {
    limit: usize,
    by_operation: BTreeMap<u64, Reservation>,
    by_frame: BTreeMap<u32, u64>,
}

impl PreSubmitDeadlineReservations {
    pub(crate) fn new(max_in_flight_operations: u16) -> Self {
        Self {
            limit: usize::from(max_in_flight_operations.max(1)),
            by_operation: BTreeMap::new(),
            by_frame: BTreeMap::new(),
        }
    }

    pub(crate) fn reserve(
        &mut self,
        frame_id: u32,
        metadata: SchedulingMetadata,
        now_unix_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.reject_expired(now_unix_ms)?;
        if frame_id == 0 || metadata.operation_id == 0 {
            return Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE requires non-zero operation and frame ids",
            ));
        }
        if metadata.deadline_unix_ms <= now_unix_ms {
            return Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE is already expired",
            ));
        }
        if self.by_operation.contains_key(&metadata.operation_id)
            || self.by_frame.contains_key(&frame_id)
        {
            return Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE reservation is duplicated",
            ));
        }
        if self.by_operation.len() >= self.limit {
            return Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE reservation limit exceeded",
            ));
        }

        self.by_operation
            .insert(metadata.operation_id, Reservation { frame_id, metadata });
        self.by_frame.insert(frame_id, metadata.operation_id);
        Ok(())
    }

    pub(crate) fn take_for_submit(
        &mut self,
        operation_id: u64,
        frame_id: u32,
        now_unix_ms: u64,
    ) -> Result<Option<SchedulingMetadata>, RuntimeError> {
        self.reject_expired(now_unix_ms)?;
        if let Some(reservation) = self.remove_operation(operation_id) {
            if reservation.frame_id != frame_id {
                return Err(RuntimeError::UnexpectedMessage(
                    "FRAME_SUBMIT does not match its pre-submit DEADLINE frame id",
                ));
            }
            return Ok(Some(reservation.metadata));
        }
        if let Some(reserved_operation_id) = self.by_frame.get(&frame_id).copied() {
            self.remove_operation(reserved_operation_id);
            return Err(RuntimeError::UnexpectedMessage(
                "FRAME_SUBMIT does not match its pre-submit DEADLINE operation id",
            ));
        }
        Ok(None)
    }

    pub(crate) fn reject_expired(&self, now_unix_ms: u64) -> Result<(), RuntimeError> {
        if self
            .by_operation
            .values()
            .any(|reservation| reservation.metadata.deadline_unix_ms <= now_unix_ms)
        {
            return Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE expired before FRAME_SUBMIT",
            ));
        }
        Ok(())
    }

    pub(crate) fn discard(&mut self, operation_id: u64, frame_id: u32) {
        if self
            .by_operation
            .get(&operation_id)
            .is_some_and(|reservation| reservation.frame_id == frame_id)
        {
            self.remove_operation(operation_id);
        }
    }

    fn remove_operation(&mut self, operation_id: u64) -> Option<Reservation> {
        let reservation = self.by_operation.remove(&operation_id)?;
        self.by_frame.remove(&reservation.frame_id);
        Some(reservation)
    }
}

pub(crate) fn current_unix_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        return js_sys::Date::now().clamp(0.0, u64::MAX as f64) as u64;
    }

    #[cfg(not(target_arch = "wasm32"))]
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deadline(operation_id: u64, deadline_unix_ms: u64) -> SchedulingMetadata {
        SchedulingMetadata {
            operation_id,
            control_sequence: operation_id,
            priority_class: 0,
            priority_delta: 0,
            deadline_unix_ms,
            flags: 0,
        }
    }

    #[test]
    fn matching_submit_consumes_the_reserved_deadline_once() {
        let mut reservations = PreSubmitDeadlineReservations::new(2);
        let metadata = deadline(11, 2_000);
        reservations.reserve(7, metadata, 1_000).unwrap();

        assert_eq!(
            reservations.take_for_submit(11, 7, 1_001).unwrap(),
            Some(metadata)
        );
        assert_eq!(reservations.take_for_submit(11, 7, 1_002).unwrap(), None);
    }

    #[test]
    fn duplicate_and_over_capacity_reservations_are_rejected() {
        let mut reservations = PreSubmitDeadlineReservations::new(1);
        reservations.reserve(7, deadline(11, 2_000), 1_000).unwrap();

        assert!(matches!(
            reservations.reserve(8, deadline(11, 2_100), 1_000),
            Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE reservation is duplicated"
            ))
        ));
        assert!(matches!(
            reservations.reserve(8, deadline(12, 2_100), 1_000),
            Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE reservation limit exceeded"
            ))
        ));
    }

    #[test]
    fn mismatched_submit_consumes_and_rejects_the_reservation() {
        let mut reservations = PreSubmitDeadlineReservations::new(2);
        reservations.reserve(7, deadline(11, 2_000), 1_000).unwrap();

        assert!(matches!(
            reservations.take_for_submit(11, 8, 1_001),
            Err(RuntimeError::UnexpectedMessage(
                "FRAME_SUBMIT does not match its pre-submit DEADLINE frame id"
            ))
        ));
        assert_eq!(reservations.take_for_submit(11, 7, 1_002).unwrap(), None);

        reservations.reserve(7, deadline(11, 2_000), 1_003).unwrap();
        assert!(matches!(
            reservations.take_for_submit(12, 7, 1_004),
            Err(RuntimeError::UnexpectedMessage(
                "FRAME_SUBMIT does not match its pre-submit DEADLINE operation id"
            ))
        ));
    }

    #[test]
    fn expired_reservations_reject_following_traffic() {
        let mut reservations = PreSubmitDeadlineReservations::new(2);
        assert!(matches!(
            reservations.reserve(7, deadline(11, 1_000), 1_000),
            Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE is already expired"
            ))
        ));

        reservations.reserve(7, deadline(11, 1_001), 1_000).unwrap();
        assert!(matches!(
            reservations.take_for_submit(11, 7, 1_001),
            Err(RuntimeError::UnexpectedMessage(
                "pre-submit DEADLINE expired before FRAME_SUBMIT"
            ))
        ));
    }
}
