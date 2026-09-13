mod map;
mod protect;
mod unmap;

use bitflags::bitflags;
use roxy_vm::Permissions;

use crate::{Syscall, errno::Errno};

pub(super) const MAP_SYSCALL: Syscall = map::SYSCALL;
pub(super) const PROTECT_SYSCALL: Syscall = protect::SYSCALL;
pub(super) const UNMAP_SYSCALL: Syscall = unmap::SYSCALL;

/// Roxy numbers protection bits from a base above Linux's `PROT_READ`/`WRITE`/`EXEC`, so a
/// Linux-valued protection is reported as a foreign numbering. `PROT_NONE` is the empty set.
const PROTECTION_BASE: u64 = 1 << 20;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MemoryProtection: u64 {
        const READ = PROTECTION_BASE;
        const WRITE = PROTECTION_BASE << 1;
        const EXECUTE = PROTECTION_BASE << 2;
    }
}

impl MemoryProtection {
    fn validate_mapping(bits: u64) -> Result<(), Errno> {
        let protection = Self::parse(bits, "vm_map.protection.unknown")?;

        if protection != Self::READ | Self::WRITE {
            return Err(unsupported("vm_map.protection", bits));
        }

        Ok(())
    }

    /// Parses raw bits using the `MemoryProtection` flags and converts supported combinations
    /// into VM permissions.
    fn parse_permissions(bits: u64) -> Result<Permissions, Errno> {
        let protection = Self::parse(bits, "vm_protect.protection.unknown")?;

        match protection {
            Self::READ => Ok(Permissions::ReadOnly),
            value if value == Self::READ | Self::WRITE => Ok(Permissions::ReadWrite),
            value if value == Self::READ | Self::EXECUTE => Ok(Permissions::ReadExecute),
            _ => Err(unsupported("vm_protect.protection", bits)),
        }
    }

    fn parse(bits: u64, operation: &str) -> Result<Self, Errno> {
        // Anything below the Roxy base is another personality's numbering.
        let foreign = bits & (PROTECTION_BASE - 1);

        if foreign != 0 {
            return Err(unsupported(operation, foreign));
        }

        let protection = Self::from_bits_retain(bits);
        let unknown = bits & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported(operation, unknown));
        }

        Ok(protection)
    }
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
