use bitflags::bitflags;

bitflags! {
    /// The file status flags of an open file description, as reported by `fcntl(F_GETFL)`.
    ///
    /// These are properties of the open file description shared by duplicated descriptors, not of
    /// a single descriptor. `StatusFlags::default()` is empty, meaning read-only access with no
    /// append or large-file mode, which is the correct baseline for the stream-like objects (pipes,
    /// sockets, terminals) that do not carry a distinct access mode.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct StatusFlags: u64 {
        const WRITE_ONLY = 0o1;
        const READ_WRITE = 0o2;
        const APPEND = 0o2000;
        const LARGE_FILE = 0o100_000;
    }
}
