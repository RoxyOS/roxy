use alloc::vec::Vec;

use roxy_memory::{PAGE_SIZE, UserAddress};
use roxy_vm::AddrSpaceHandle;

use crate::errno::Errno;

pub(crate) fn read_c_string(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    max_length: usize,
) -> Result<Vec<u8>, Errno> {
    let mut output = Vec::new();

    while output.len() < max_length {
        let current = address
            .checked_add(u64::try_from(output.len()).map_err(|_| Errno::Fault)?)
            .ok_or(Errno::Fault)?;
        let page_remaining = usize::try_from(PAGE_SIZE - current.as_u64() % PAGE_SIZE).unwrap();
        let length = page_remaining.min(max_length - output.len());
        let start = output.len();

        output.resize(start + length, 0);
        addrspace
            .read_bytes(current, &mut output[start..])
            .map_err(|_| Errno::Fault)?;

        if let Some(terminator) = output[start..].iter().position(|byte| *byte == 0) {
            output.truncate(start + terminator);

            return Ok(output);
        }
    }

    Err(Errno::NameTooLong)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::num::NonZeroUsize;

    use roxy_memory::{UserAddress, UserPage};
    use roxy_test::kernel_test;
    use roxy_vm::{AddrSpace, Permissions, UserRegion};

    use super::read_c_string;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::user-c-string", reads_page_spanning_string, {
        let mut addrspace = AddrSpace::new().unwrap();
        let region = UserRegion::new(
            UserPage::new(UserAddress::new(0x40_0000).unwrap()).unwrap(),
            NonZeroUsize::new(2).unwrap(),
        )
        .unwrap();

        addrspace
            .map_zeroed(region, Permissions::ReadWrite)
            .unwrap();

        let address = UserAddress::new(0x40_0ffc).unwrap();

        addrspace.write_bytes(address, b"/file\0").unwrap();

        let addrspace = addrspace.into_handle();

        assert_eq!(read_c_string(&addrspace, address, 4096).unwrap(), b"/file");
        assert_eq!(
            read_c_string(&addrspace, UserAddress::new(0x40_0000).unwrap(), 4),
            Ok(alloc::vec![])
        );
        assert_eq!(
            read_c_string(&addrspace, UserAddress::new(0x60_0000).unwrap(), 4),
            Err(Errno::Fault)
        );
    });

    kernel_test!(
        "roxy-syscall::unterminated-user-string",
        rejects_unterminated_string,
        {
            let mut addrspace = AddrSpace::new().unwrap();
            let region = UserRegion::new(
                UserPage::new(UserAddress::new(0x50_0000).unwrap()).unwrap(),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();

            addrspace
                .map_zeroed(region, Permissions::ReadWrite)
                .unwrap();

            let address = UserAddress::new(0x50_0000).unwrap();

            addrspace.write_bytes(address, b"file").unwrap();

            assert_eq!(
                read_c_string(&addrspace.into_handle(), address, 4),
                Err(Errno::NameTooLong)
            );
        }
    );
}
