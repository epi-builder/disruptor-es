use crate::UserId;
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};

/// User aggregate marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct User;

/// User lifecycle status.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum UserStatus {
    /// User has not been registered.
    #[default]
    Unregistered,
    /// User may place orders.
    Active,
    /// User may not place orders.
    Inactive,
}

/// User aggregate state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UserState {
    /// Registered user identity, if any.
    pub user_id: Option<UserId>,
    /// Registered user email, if any.
    pub email: Option<String>,
    /// Registered user display name, if any.
    pub display_name: Option<String>,
    /// Current user lifecycle status.
    pub status: UserStatus,
}

/// Commands accepted by the user aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum UserCommand {
    /// Registers a new user.
    RegisterUser { user_id: UserId, email: String, display_name: String },
    /// Activates a registered user.
    ActivateUser { user_id: UserId },
    /// Deactivates an active user.
    DeactivateUser { user_id: UserId },
}

/// Events emitted by the user aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum UserEvent {
    /// User was registered.
    UserRegistered { user_id: UserId, email: String, display_name: String },
    /// User was activated.
    UserActivated { user_id: UserId },
    /// User was deactivated.
    UserDeactivated { user_id: UserId },
}

/// Replies returned by user commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserReply {
    /// Registration succeeded.
    Registered {
        /// Registered user.
        user_id: UserId,
    },
    /// Activation succeeded.
    Activated {
        /// Activated user.
        user_id: UserId,
    },
    /// Deactivation succeeded.
    Deactivated {
        /// Deactivated user.
        user_id: UserId,
    },
}

/// User command validation errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UserError {
    /// Email must not be empty.
    #[error("user email cannot be empty")]
    EmptyEmail,
    /// Display name must not be empty.
    #[error("user display name cannot be empty")]
    EmptyDisplayName,
    /// User is already registered.
    #[error("user is already registered")]
    AlreadyRegistered,
    /// User must be registered before the operation.
    #[error("user is not registered")]
    NotRegistered,
    /// User is already active.
    #[error("user is already active")]
    AlreadyActive,
    /// User is already inactive.
    #[error("user is already inactive")]
    AlreadyInactive,
}

impl Aggregate for User {
    type State = UserState;
    type Command = UserCommand;
    type Event = UserEvent;
    type Reply = UserReply;
    type Error = UserError;

    fn stream_id(command: &Self::Command) -> StreamId {
        StreamId::new(stream_key(command)).expect("user id is a valid stream id")
    }

    fn partition_key(command: &Self::Command) -> PartitionKey {
        PartitionKey::new(stream_key(command)).expect("user id is a valid partition key")
    }

    fn expected_revision(command: &Self::Command) -> ExpectedRevision {
        match command {
            UserCommand::RegisterUser { .. } => ExpectedRevision::NoStream,
            UserCommand::ActivateUser { .. } | UserCommand::DeactivateUser { .. } => {
                ExpectedRevision::Any
            }
        }
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        match command {
            UserCommand::RegisterUser {
                user_id,
                email,
                display_name,
            } => {
                if email.is_empty() {
                    return Err(UserError::EmptyEmail);
                }
                if display_name.is_empty() {
                    return Err(UserError::EmptyDisplayName);
                }
                if state.status != UserStatus::Unregistered {
                    return Err(UserError::AlreadyRegistered);
                }

                Ok(Decision::new(
                    vec![UserEvent::UserRegistered {
                        user_id: user_id.clone(),
                        email,
                        display_name,
                    }],
                    UserReply::Registered { user_id },
                ))
            }
            UserCommand::ActivateUser { user_id } => match state.status {
                UserStatus::Unregistered => Err(UserError::NotRegistered),
                UserStatus::Active => Err(UserError::AlreadyActive),
                UserStatus::Inactive => Ok(Decision::new(
                    vec![UserEvent::UserActivated {
                        user_id: user_id.clone(),
                    }],
                    UserReply::Activated { user_id },
                )),
            },
            UserCommand::DeactivateUser { user_id } => match state.status {
                UserStatus::Unregistered => Err(UserError::NotRegistered),
                UserStatus::Inactive => Err(UserError::AlreadyInactive),
                UserStatus::Active => Ok(Decision::new(
                    vec![UserEvent::UserDeactivated {
                        user_id: user_id.clone(),
                    }],
                    UserReply::Deactivated { user_id },
                )),
            },
        }
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        match event {
            UserEvent::UserRegistered {
                user_id,
                email,
                display_name,
            } => {
                state.user_id = Some(user_id.clone());
                state.email = Some(email.clone());
                state.display_name = Some(display_name.clone());
                state.status = UserStatus::Inactive;
            }
            UserEvent::UserActivated { user_id } => {
                state.user_id = Some(user_id.clone());
                state.status = UserStatus::Active;
            }
            UserEvent::UserDeactivated { user_id } => {
                state.user_id = Some(user_id.clone());
                state.status = UserStatus::Inactive;
            }
        }
    }
}

fn stream_key(command: &UserCommand) -> String {
    match command {
        UserCommand::RegisterUser { user_id, .. }
        | UserCommand::ActivateUser { user_id }
        | UserCommand::DeactivateUser { user_id } => format!("user-{}", user_id.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_core::{CommandMetadata, TenantId};
    use es_kernel::Aggregate;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(11),
            correlation_id: Uuid::from_u128(12),
            causation_id: None,
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        }
    }

    fn user_id() -> UserId {
        UserId::new("user-1").expect("user id")
    }

    #[test]
    fn register_user_emits_registered_event() {
        let user_id = user_id();

        let decision = User::decide(
            &UserState::default(),
            UserCommand::RegisterUser {
                user_id: user_id.clone(),
                email: "a@example.test".to_owned(),
                display_name: "Ada".to_owned(),
            },
            &metadata(),
        )
        .expect("register user");

        assert_eq!(
            vec![UserEvent::UserRegistered {
                user_id: user_id.clone(),
                email: "a@example.test".to_owned(),
                display_name: "Ada".to_owned(),
            }],
            decision.events
        );
        assert_eq!(UserReply::Registered { user_id }, decision.reply);
    }

    #[test]
    fn user_lifecycle_activate_and_deactivate_is_replayable() {
        let user_id = user_id();
        let registered = UserEvent::UserRegistered {
            user_id: user_id.clone(),
            email: "a@example.test".to_owned(),
            display_name: "Ada".to_owned(),
        };
        let registered_state = es_kernel::replay::<User>([registered.clone()]);

        let activation = User::decide(
            &registered_state,
            UserCommand::ActivateUser {
                user_id: user_id.clone(),
            },
            &metadata(),
        )
        .expect("activate user");

        assert_eq!(
            vec![UserEvent::UserActivated {
                user_id: user_id.clone(),
            }],
            activation.events
        );
        assert_eq!(
            UserReply::Activated {
                user_id: user_id.clone(),
            },
            activation.reply
        );

        let active_state =
            es_kernel::replay::<User>([registered.clone(), activation.events[0].clone()]);
        assert_eq!(UserStatus::Active, active_state.status);

        let deactivation = User::decide(
            &active_state,
            UserCommand::DeactivateUser {
                user_id: user_id.clone(),
            },
            &metadata(),
        )
        .expect("deactivate user");

        assert_eq!(
            vec![UserEvent::UserDeactivated {
                user_id: user_id.clone(),
            }],
            deactivation.events
        );
        assert_eq!(
            UserReply::Deactivated {
                user_id: user_id.clone(),
            },
            deactivation.reply
        );

        let inactive_state = es_kernel::replay::<User>([
            registered,
            activation.events[0].clone(),
            deactivation.events[0].clone(),
        ]);

        assert_eq!(
            UserState {
                user_id: Some(user_id),
                email: Some("a@example.test".to_owned()),
                display_name: Some("Ada".to_owned()),
                status: UserStatus::Inactive,
            },
            inactive_state
        );
    }

    #[test]
    fn user_rejects_invalid_lifecycle_transitions() {
        let user_id = user_id();

        assert_eq!(
            UserError::NotRegistered,
            User::decide(
                &UserState::default(),
                UserCommand::ActivateUser {
                    user_id: user_id.clone(),
                },
                &metadata(),
            )
            .expect_err("activation requires registration")
        );

        let registered = UserEvent::UserRegistered {
            user_id: user_id.clone(),
            email: "a@example.test".to_owned(),
            display_name: "Ada".to_owned(),
        };
        let registered_state = es_kernel::replay::<User>([registered.clone()]);

        assert_eq!(
            UserError::AlreadyRegistered,
            User::decide(
                &registered_state,
                UserCommand::RegisterUser {
                    user_id: user_id.clone(),
                    email: "a@example.test".to_owned(),
                    display_name: "Ada".to_owned(),
                },
                &metadata(),
            )
            .expect_err("duplicate registration")
        );

        let active_state = es_kernel::replay::<User>([
            registered.clone(),
            UserEvent::UserActivated {
                user_id: user_id.clone(),
            },
        ]);

        assert_eq!(
            UserError::AlreadyActive,
            User::decide(
                &active_state,
                UserCommand::ActivateUser {
                    user_id: user_id.clone(),
                },
                &metadata(),
            )
            .expect_err("duplicate activation")
        );

        let inactive_state = es_kernel::replay::<User>([
            registered,
            UserEvent::UserDeactivated {
                user_id: user_id.clone(),
            },
        ]);

        assert_eq!(
            UserError::AlreadyInactive,
            User::decide(
                &inactive_state,
                UserCommand::DeactivateUser { user_id },
                &metadata(),
            )
            .expect_err("duplicate deactivation")
        );
    }
}
