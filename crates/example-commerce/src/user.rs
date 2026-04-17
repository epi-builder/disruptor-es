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
