mod kernel;
mod pagetable;

pub(crate) use kernel::X86_64KernelPageTableBackend;
pub(crate) use pagetable::X86_64AddrSpacePageTable;
