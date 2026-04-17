use crate::UserId;

/// User aggregate marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct User;

/// User lifecycle status.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum UserStatus {
    /// User has not been registered.
    #[default]
    Unregistered,
    /// User has been registered but is not active yet.
    Registered,
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
    /// Current user lifecycle status.
    pub status: UserStatus,
}

/// Commands accepted by the user aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserCommand {}

/// Events emitted by the user aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserEvent {}

/// Replies returned by user commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserReply {}

/// User command validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserError {}

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
