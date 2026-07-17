use core::num::NonZeroUsize;

use roxy_memory::UserPage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserRegion {
    pub start: UserPage,
    pub page_count: NonZeroUsize,
}

impl UserRegion {
    #[must_use]
    pub fn new(start: UserPage, page_count: NonZeroUsize) -> Option<Self> {
        start.checked_add(page_count.get() - 1)?;
        Some(Self { start, page_count })
    }

    pub(crate) fn pages(self) -> impl Iterator<Item = UserPage> {
        (0..self.page_count.get()).map(move |index| self.start.checked_add(index).unwrap())
    }
}
