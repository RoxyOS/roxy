#[derive(Clone, Copy)]
pub(super) enum Side {
    First,
    Second,
}

impl Side {
    pub(super) const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }

    pub(super) const fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}
