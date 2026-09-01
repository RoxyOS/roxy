#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultAction {
    Terminate,
    Stop,
    Continue,
    Ignore,
    Unsupported,
}
