#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultAction {
    Terminate,
    Ignore,
    Unsupported,
}
