use roxy_memory::{UserAddress, UserPage};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserStack {
    pub bottom: UserAddress,
    pub top: UserAddress,
    pub guard_page: UserPage,
}

impl UserStack {
    pub(crate) const fn new(bottom: UserAddress, top: UserAddress, guard_page: UserPage) -> Self {
        Self {
            bottom,
            top,
            guard_page,
        }
    }
}
