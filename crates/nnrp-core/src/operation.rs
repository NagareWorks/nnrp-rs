use std::collections::{BTreeMap, BTreeSet};

use crate::{CancelScope, MessageType, NnrpError, OperationState, SchedulingMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationDescriptor {
    pub session_id: u32,
    pub operation_id: u64,
    pub parent_operation_id: Option<u64>,
    pub operation_group_id: Option<u64>,
}

impl OperationDescriptor {
    pub fn new(session_id: u32, operation_id: u64) -> Self {
        Self {
            session_id,
            operation_id,
            parent_operation_id: None,
            operation_group_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecord {
    pub descriptor: OperationDescriptor,
    pub state: OperationState,
    pub schedule: OperationSchedule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationCancelRequest {
    pub session_id: u32,
    pub operation_id: u64,
    pub cancel_scope: CancelScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OperationSchedule {
    pub priority_class: u16,
    pub priority_delta: i16,
    pub deadline_unix_ms: u64,
    pub expire_at_unix_ms: u64,
    pub update_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OperationRegistry {
    operations: BTreeMap<u64, OperationRecord>,
}

impl OperationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    pub fn operation(&self, operation_id: u64) -> Option<&OperationRecord> {
        self.operations.get(&operation_id)
    }

    pub fn register(&mut self, descriptor: OperationDescriptor) -> Result<(), NnrpError> {
        validate_descriptor_shape(&descriptor)?;
        if self.operations.contains_key(&descriptor.operation_id) {
            return Err(NnrpError::OperationAlreadyExists(descriptor.operation_id));
        }

        if let Some(parent_operation_id) = descriptor.parent_operation_id {
            let parent = self
                .operations
                .get(&parent_operation_id)
                .ok_or(NnrpError::UnknownOperation(parent_operation_id))?;
            if parent.descriptor.session_id != descriptor.session_id {
                return Err(NnrpError::InvalidOperationRelationship {
                    rule: "parent operation must belong to the same session",
                });
            }
        }

        self.operations.insert(
            descriptor.operation_id,
            OperationRecord {
                descriptor,
                state: OperationState::Accepted,
                schedule: OperationSchedule::default(),
            },
        );
        Ok(())
    }

    pub fn transition(
        &mut self,
        operation_id: u64,
        next_state: OperationState,
    ) -> Result<(), NnrpError> {
        let record = self
            .operations
            .get_mut(&operation_id)
            .ok_or(NnrpError::UnknownOperation(operation_id))?;
        if !record.state.can_transition_to(next_state) {
            return Err(NnrpError::InvalidOperationTransition {
                from: record.state,
                to: next_state,
            });
        }

        record.state = next_state;
        Ok(())
    }

    pub fn complete(&mut self, operation_id: u64) -> Result<(), NnrpError> {
        let record = self
            .operations
            .get_mut(&operation_id)
            .ok_or(NnrpError::UnknownOperation(operation_id))?;
        if record.state.is_terminal() {
            return Err(NnrpError::InvalidOperationTransition {
                from: record.state,
                to: OperationState::Completed,
            });
        }

        if record.state == OperationState::Accepted {
            record.state = OperationState::Running;
        }
        if !record.state.can_transition_to(OperationState::Completed) {
            return Err(NnrpError::InvalidOperationTransition {
                from: record.state,
                to: OperationState::Completed,
            });
        }

        record.state = OperationState::Completed;
        Ok(())
    }

    pub fn apply_scheduling_update(
        &mut self,
        session_id: u32,
        message_type: MessageType,
        metadata: SchedulingMetadata,
    ) -> Result<OperationSchedule, NnrpError> {
        let record = self
            .operations
            .get_mut(&metadata.operation_id)
            .ok_or(NnrpError::UnknownOperation(metadata.operation_id))?;
        if record.descriptor.session_id != session_id {
            return Err(NnrpError::InvalidOperationRelationship {
                rule: "scheduling update session_id must match the target operation",
            });
        }
        if record.state.is_terminal() {
            return Err(NnrpError::InvalidOperationTransition {
                from: record.state,
                to: record.state,
            });
        }

        record.schedule.update_sequence = metadata.control_sequence;
        match message_type {
            MessageType::PriorityUpdate => {
                record.schedule.priority_class = metadata.priority_class;
                record.schedule.priority_delta = metadata.priority_delta;
            }
            MessageType::Deadline => {
                record.schedule.deadline_unix_ms = metadata.deadline_unix_ms;
            }
            MessageType::ExpireAt => {
                record.schedule.expire_at_unix_ms = metadata.deadline_unix_ms;
            }
            _ => {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "scheduling update requires PRIORITY_UPDATE, DEADLINE, or EXPIRE_AT",
                });
            }
        }

        Ok(record.schedule)
    }

    pub fn cancel(&mut self, request: OperationCancelRequest) -> Result<Vec<u64>, NnrpError> {
        let target = self
            .operations
            .get(&request.operation_id)
            .ok_or(NnrpError::UnknownOperation(request.operation_id))?;
        if target.descriptor.session_id != request.session_id {
            return Err(NnrpError::InvalidOperationRelationship {
                rule: "cancel request session_id must match the target operation",
            });
        }

        let mut operation_ids = match request.cancel_scope {
            CancelScope::Operation => vec![request.operation_id],
            CancelScope::Subtree => self.subtree_operation_ids(request.operation_id),
            CancelScope::Group => self.group_operation_ids(target.descriptor.operation_group_id)?,
            CancelScope::Session => self.session_operation_ids(request.session_id),
        };
        operation_ids.sort_unstable();

        let mut cancelled = Vec::new();
        for operation_id in operation_ids {
            let record = self
                .operations
                .get_mut(&operation_id)
                .expect("collected operation id should exist");
            if !record.state.is_terminal() {
                record.state = OperationState::Cancelled;
                cancelled.push(operation_id);
            }
        }

        Ok(cancelled)
    }

    fn subtree_operation_ids(&self, root_operation_id: u64) -> Vec<u64> {
        let mut collected = BTreeSet::new();
        let mut stack = vec![root_operation_id];

        while let Some(operation_id) = stack.pop() {
            if !collected.insert(operation_id) {
                continue;
            }

            for record in self.operations.values() {
                if record.descriptor.parent_operation_id == Some(operation_id) {
                    stack.push(record.descriptor.operation_id);
                }
            }
        }

        collected.into_iter().collect()
    }

    fn group_operation_ids(&self, operation_group_id: Option<u64>) -> Result<Vec<u64>, NnrpError> {
        let operation_group_id =
            operation_group_id.ok_or(NnrpError::InvalidOperationRelationship {
                rule: "group cancel requires the target operation to have an operation_group_id",
            })?;

        Ok(self
            .operations
            .values()
            .filter(|record| record.descriptor.operation_group_id == Some(operation_group_id))
            .map(|record| record.descriptor.operation_id)
            .collect())
    }

    fn session_operation_ids(&self, session_id: u32) -> Vec<u64> {
        self.operations
            .values()
            .filter(|record| record.descriptor.session_id == session_id)
            .map(|record| record.descriptor.operation_id)
            .collect()
    }
}

fn validate_descriptor_shape(descriptor: &OperationDescriptor) -> Result<(), NnrpError> {
    if descriptor.session_id == 0 {
        return Err(NnrpError::InvalidOperationRelationship {
            rule: "operation session_id must be non-zero",
        });
    }
    if descriptor.operation_id == 0 {
        return Err(NnrpError::InvalidOperationRelationship {
            rule: "operation_id must be non-zero",
        });
    }
    if descriptor.parent_operation_id == Some(descriptor.operation_id) {
        return Err(NnrpError::InvalidOperationRelationship {
            rule: "operation cannot be its own parent",
        });
    }
    if descriptor.operation_group_id == Some(0) {
        return Err(NnrpError::InvalidOperationRelationship {
            rule: "operation_group_id must be non-zero when present",
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{CancelScope, MessageType, NnrpError, OperationState, SchedulingMetadata};

    use super::{OperationCancelRequest, OperationDescriptor, OperationRegistry};

    #[test]
    fn registers_operation_tree_and_groups() {
        let mut registry = OperationRegistry::new();
        registry.register(grouped(1, None, 7)).unwrap();
        registry.register(grouped(2, Some(1), 7)).unwrap();
        registry.register(grouped(3, Some(1), 8)).unwrap();

        assert_eq!(registry.operation_count(), 3);
        assert_eq!(
            registry
                .operation(2)
                .unwrap()
                .descriptor
                .parent_operation_id,
            Some(1)
        );
        assert_eq!(
            registry.operation(3).unwrap().descriptor.operation_group_id,
            Some(8)
        );
    }

    #[test]
    fn rejects_invalid_operation_relationships() {
        let mut registry = OperationRegistry::new();

        assert_eq!(
            registry.register(OperationDescriptor::new(0, 1)),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "operation session_id must be non-zero"
            })
        );
        assert_eq!(
            registry.register(OperationDescriptor::new(42, 0)),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "operation_id must be non-zero"
            })
        );

        let mut self_parent = OperationDescriptor::new(42, 1);
        self_parent.parent_operation_id = Some(1);
        assert_eq!(
            registry.register(self_parent),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "operation cannot be its own parent"
            })
        );

        let mut zero_group = OperationDescriptor::new(42, 1);
        zero_group.operation_group_id = Some(0);
        assert_eq!(
            registry.register(zero_group),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "operation_group_id must be non-zero when present"
            })
        );
    }

    #[test]
    fn requires_parent_to_exist_in_same_session() {
        let mut registry = OperationRegistry::new();
        let mut child = OperationDescriptor::new(42, 2);
        child.parent_operation_id = Some(1);
        assert_eq!(
            registry.register(child),
            Err(NnrpError::UnknownOperation(1))
        );

        registry.register(OperationDescriptor::new(7, 1)).unwrap();
        assert_eq!(
            registry.register(child),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "parent operation must belong to the same session"
            })
        );
    }

    #[test]
    fn enforces_lifecycle_transitions() {
        let mut registry = OperationRegistry::new();
        registry.register(OperationDescriptor::new(42, 1)).unwrap();

        registry.transition(1, OperationState::Running).unwrap();
        registry.transition(1, OperationState::Partial).unwrap();
        registry.transition(1, OperationState::Completed).unwrap();

        assert_eq!(
            registry.transition(1, OperationState::Running),
            Err(NnrpError::InvalidOperationTransition {
                from: OperationState::Completed,
                to: OperationState::Running
            })
        );
        assert_eq!(
            registry.transition(99, OperationState::Running),
            Err(NnrpError::UnknownOperation(99))
        );
    }

    #[test]
    fn completes_active_operations_and_rejects_terminal_or_unknown_records() {
        let mut registry = OperationRegistry::new();
        registry.register(OperationDescriptor::new(42, 1)).unwrap();
        registry.register(OperationDescriptor::new(42, 2)).unwrap();
        registry.register(OperationDescriptor::new(42, 3)).unwrap();
        registry.transition(2, OperationState::Running).unwrap();
        registry.transition(2, OperationState::Cancelled).unwrap();
        registry.transition(3, OperationState::Running).unwrap();
        registry.transition(3, OperationState::WaitingTool).unwrap();

        assert_eq!(registry.complete(1), Ok(()));
        assert_eq!(
            registry.operation(1).unwrap().state,
            OperationState::Completed
        );
        assert_eq!(
            registry.complete(2),
            Err(NnrpError::InvalidOperationTransition {
                from: OperationState::Cancelled,
                to: OperationState::Completed,
            })
        );
        assert_eq!(
            registry.complete(3),
            Err(NnrpError::InvalidOperationTransition {
                from: OperationState::WaitingTool,
                to: OperationState::Completed,
            })
        );
        assert_eq!(registry.complete(99), Err(NnrpError::UnknownOperation(99)));
    }

    #[test]
    fn cancels_operation_subtree_group_and_session_scopes() {
        let mut registry = OperationRegistry::new();
        registry.register(grouped(1, None, 7)).unwrap();
        registry.register(grouped(2, Some(1), 7)).unwrap();
        registry.register(grouped(3, Some(2), 8)).unwrap();
        registry.register(grouped(4, None, 7)).unwrap();
        registry.register(grouped(5, None, 9)).unwrap();
        registry.transition(5, OperationState::Running).unwrap();
        registry.transition(5, OperationState::Completed).unwrap();

        assert_eq!(
            registry.cancel(OperationCancelRequest {
                session_id: 42,
                operation_id: 2,
                cancel_scope: CancelScope::Subtree,
            }),
            Ok(vec![2, 3])
        );
        assert_eq!(
            registry.operation(1).unwrap().state,
            OperationState::Accepted
        );

        assert_eq!(
            registry.cancel(OperationCancelRequest {
                session_id: 42,
                operation_id: 1,
                cancel_scope: CancelScope::Group,
            }),
            Ok(vec![1, 4])
        );

        assert_eq!(
            registry.cancel(OperationCancelRequest {
                session_id: 42,
                operation_id: 5,
                cancel_scope: CancelScope::Session,
            }),
            Ok(Vec::<u64>::new())
        );
        assert_eq!(
            registry.operation(5).unwrap().state,
            OperationState::Completed
        );
    }

    #[test]
    fn rejects_invalid_cancel_requests() {
        let mut registry = OperationRegistry::new();
        registry.register(OperationDescriptor::new(42, 1)).unwrap();

        assert_eq!(
            registry.cancel(OperationCancelRequest {
                session_id: 7,
                operation_id: 1,
                cancel_scope: CancelScope::Operation,
            }),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "cancel request session_id must match the target operation"
            })
        );

        assert_eq!(
            registry.cancel(OperationCancelRequest {
                session_id: 42,
                operation_id: 1,
                cancel_scope: CancelScope::Group,
            }),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "group cancel requires the target operation to have an operation_group_id"
            })
        );
        assert_eq!(
            registry.cancel(OperationCancelRequest {
                session_id: 42,
                operation_id: 99,
                cancel_scope: CancelScope::Operation,
            }),
            Err(NnrpError::UnknownOperation(99))
        );
    }

    #[test]
    fn applies_scheduling_updates_to_operation_records() {
        let mut registry = OperationRegistry::new();
        registry.register(OperationDescriptor::new(42, 1)).unwrap();

        assert_eq!(
            registry.apply_scheduling_update(
                42,
                MessageType::PriorityUpdate,
                scheduling(1, 10, 3, -1, 0),
            ),
            Ok(super::OperationSchedule {
                priority_class: 3,
                priority_delta: -1,
                update_sequence: 10,
                ..super::OperationSchedule::default()
            })
        );

        assert_eq!(
            registry.apply_scheduling_update(
                42,
                MessageType::Deadline,
                scheduling(1, 11, 0, 0, 50)
            ),
            Ok(super::OperationSchedule {
                priority_class: 3,
                priority_delta: -1,
                deadline_unix_ms: 50,
                update_sequence: 11,
                ..super::OperationSchedule::default()
            })
        );

        assert_eq!(
            registry.apply_scheduling_update(
                42,
                MessageType::ExpireAt,
                scheduling(1, 12, 0, 0, 60),
            ),
            Ok(super::OperationSchedule {
                priority_class: 3,
                priority_delta: -1,
                deadline_unix_ms: 50,
                expire_at_unix_ms: 60,
                update_sequence: 12,
            })
        );
        assert_eq!(
            registry.operation(1).unwrap().schedule.expire_at_unix_ms,
            60
        );
    }

    #[test]
    fn rejects_scheduling_updates_for_wrong_session_unknown_operation_and_terminal_records() {
        let mut registry = OperationRegistry::new();
        registry.register(OperationDescriptor::new(42, 1)).unwrap();

        assert_eq!(
            registry.apply_scheduling_update(7, MessageType::Deadline, scheduling(1, 1, 0, 0, 5)),
            Err(NnrpError::InvalidOperationRelationship {
                rule: "scheduling update session_id must match the target operation"
            })
        );
        assert_eq!(
            registry.apply_scheduling_update(42, MessageType::Deadline, scheduling(9, 1, 0, 0, 5)),
            Err(NnrpError::UnknownOperation(9))
        );

        registry.transition(1, OperationState::Running).unwrap();
        registry.transition(1, OperationState::Completed).unwrap();
        assert_eq!(
            registry.apply_scheduling_update(42, MessageType::Deadline, scheduling(1, 1, 0, 0, 5)),
            Err(NnrpError::InvalidOperationTransition {
                from: OperationState::Completed,
                to: OperationState::Completed,
            })
        );
    }

    fn grouped(
        operation_id: u64,
        parent_operation_id: Option<u64>,
        operation_group_id: u64,
    ) -> OperationDescriptor {
        OperationDescriptor {
            session_id: 42,
            operation_id,
            parent_operation_id,
            operation_group_id: Some(operation_group_id),
        }
    }

    fn scheduling(
        operation_id: u64,
        control_sequence: u64,
        priority_class: u16,
        priority_delta: i16,
        deadline_unix_ms: u64,
    ) -> SchedulingMetadata {
        SchedulingMetadata {
            operation_id,
            control_sequence,
            flags: 0,
            priority_class,
            priority_delta,
            deadline_unix_ms,
        }
    }
}
