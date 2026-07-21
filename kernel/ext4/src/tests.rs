use alloc::sync::Arc;

use roxy_block::RamDisk;
use roxy_vfs::{
    CreationMode, FilePermissions, FileType, OpenAccess, OpenOptions, SeekFrom, Vfs, VfsError,
    ResolvedPath,
};
use spin::Once;

use crate::Ext4FileSystem;

static IMAGE: &[u8] = include_bytes!("../../../target/roxy/rootfs.img");
static DEVICE: Once<RamDisk> = Once::new();

roxy_test::kernel_test!(
    "roxy-ext4::vfs-file-directory-and-link-operations",
    ext4_vfs_operations,
    {
        let device = DEVICE.call_once(|| RamDisk::new(IMAGE).unwrap());
        let filesystem = Arc::new(Ext4FileSystem::load(device).unwrap());
        let vfs = Vfs::new();

        vfs.mount(ResolvedPath::root(), filesystem.clone()).unwrap();

        assert_eq!(
            vfs.metadata(&path(b"/")).unwrap().file_type,
            FileType::Directory
        );
        vfs.mkdir(&path(b"/a")).unwrap();
        vfs.mkdir(&path(b"/b")).unwrap();

        let options = OpenOptions {
            access: OpenAccess::ReadWrite,
            creation: CreationMode::CreateNew,
            permissions: FilePermissions::new(0o640).unwrap(),
            append: false,
            truncate: false,
        };

        let mut file = vfs.open(&path(b"/a/file"), options).unwrap();

        assert_eq!(
            filesystem
                .resolve_inode(&ResolvedPath::resolve(b"/a/file").unwrap(), true)
                .unwrap()
                .mode()
                .bits()
                & 0o777,
            0o640
        );

        assert_eq!(file.write(b"hello").unwrap(), 5);
        assert_eq!(file.seek(SeekFrom::Start(0)).unwrap(), 0);

        let mut contents = [0_u8; 5];

        assert_eq!(file.read(&mut contents).unwrap(), 5);
        assert_eq!(&contents, b"hello");
        drop(file);

        vfs.rename(&path(b"/a/file"), &path(b"/b/file"))
            .unwrap();
        vfs.hard_link(&path(b"/b/file"), &path(b"/hard"))
            .unwrap();
        vfs.symlink(b"/hard", &path(b"/sym")).unwrap();
        assert_eq!(vfs.read_link(&path(b"/sym")).unwrap(), b"/hard");
        vfs.rename(&path(b"/sym"), &path(b"/a/sym")).unwrap();
        assert_eq!(vfs.read_link(&path(b"/a/sym")).unwrap(), b"/hard");

        let mut hard = vfs
            .open(
                &path(b"/hard"),
                OpenOptions {
                    access: OpenAccess::ReadWrite,
                    ..OpenOptions::read_only()
                },
            )
            .unwrap();

        hard.truncate(2).unwrap();
        hard.seek(SeekFrom::Start(0)).unwrap();

        let mut shortened = [0_u8; 2];

        assert_eq!(hard.read(&mut shortened).unwrap(), 2);
        assert_eq!(&shortened, b"he");
        hard.sync().unwrap();
        drop(hard);

        let entries = vfs.read_dir(&path(b"/b")).unwrap();

        assert!(entries.iter().any(|entry| entry.name == b"file"));

        vfs.unlink(&path(b"/b/file")).unwrap();
        assert_eq!(vfs.metadata(&path(b"/hard")).unwrap().size, 2);

        vfs.mkdir(&path(b"/old")).unwrap();
        vfs.rename(&path(b"/old"), &path(b"/new")).unwrap();
        vfs.mkdir(&path(b"/a/dir")).unwrap();

        assert_eq!(
            vfs.rename(&path(b"/a/dir"), &path(b"/b/dir")),
            Err(VfsError::Unsupported)
        );

        vfs.rmdir(&path(b"/a/dir")).unwrap();
        vfs.rmdir(&path(b"/new")).unwrap();
        vfs.unlink(&path(b"/a/sym")).unwrap();
        vfs.unlink(&path(b"/hard")).unwrap();
        vfs.rmdir(&path(b"/a")).unwrap();
        vfs.rmdir(&path(b"/b")).unwrap();

        vfs.sync().unwrap();
    }
);

fn path(bytes: &[u8]) -> ResolvedPath {
    ResolvedPath::resolve(bytes).unwrap()
}
