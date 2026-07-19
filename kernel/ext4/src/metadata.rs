use ext4plus::{FileType as Ext4FileType, inode::Inode};
use roxy_vfs::{FilePermissions, FileType, Metadata};

pub(crate) fn from_inode(inode: &Inode) -> Metadata {
    let metadata = inode.metadata();

    Metadata {
        file_id: u64::from(inode.index.get()),
        file_type: map_file_type(metadata.file_type),
        permissions: FilePermissions::new(metadata.mode.bits() & 0o777).unwrap(),
        size: metadata.size_in_bytes,
        hard_links: u32::from(metadata.links_count),
    }
}

pub(crate) fn map_file_type(file_type: Ext4FileType) -> FileType {
    match file_type {
        Ext4FileType::Regular => FileType::Regular,
        Ext4FileType::Directory => FileType::Directory,
        Ext4FileType::Symlink => FileType::Symlink,
        Ext4FileType::BlockDevice => FileType::BlockDevice,
        Ext4FileType::CharacterDevice => FileType::CharacterDevice,
        Ext4FileType::Fifo => FileType::Fifo,
        Ext4FileType::Socket => FileType::Socket,
    }
}
