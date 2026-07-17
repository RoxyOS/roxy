mod addrspace;
mod kernel;

pub(crate) use addrspace::X86_64AddrSpacePageTable;
pub(crate) use kernel::X86_64KernelPageTableBackend;
