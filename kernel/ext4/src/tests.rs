use roxy_vfs::{
    CreationMode, FilePermissions, FileType, OpenAccess, OpenOptions, SeekFrom, VfsError,
    hard_link, metadata, mkdir, open, read_dir, read_link, rename, rmdir, symlink,
    symlink_metadata, sync, unlink,
};

roxy_test::kernel_test!(
    "roxy-ext4::vfs-file-directory-and-link-operations",
    ext4_vfs_operations,
    {
        assert_eq!(metadata(b"/").unwrap().file_type, FileType::Directory);
        mkdir(b"/a", FilePermissions::new(0o750).unwrap()).unwrap();
        mkdir(b"/b", FilePermissions::DEFAULT_DIRECTORY).unwrap();
        assert_eq!(metadata(b"/a").unwrap().permissions.bits(), 0o750);

        let options = OpenOptions {
            access: OpenAccess::ReadWrite,
            creation: CreationMode::CreateNew,
            permissions: FilePermissions::new(0o640).unwrap(),
            append: false,
            truncate: false,
            no_follow: false,
        };

        let mut file = open(b"/a/file", options).unwrap();

        assert_eq!(metadata(b"/a/file").unwrap().permissions.bits(), 0o640);

        assert_eq!(file.write(b"hello").unwrap(), 5);
        assert_eq!(file.seek(SeekFrom::Start(0)).unwrap(), 0);

        let mut contents = [0_u8; 5];

        assert_eq!(file.read(&mut contents).unwrap(), 5);
        assert_eq!(&contents, b"hello");
        drop(file);

        rename(b"/a/file", b"/b/file").unwrap();
        hard_link(b"/b/file", b"/hard").unwrap();
        symlink(b"/hard", b"/sym").unwrap();
        assert_eq!(metadata(b"/sym").unwrap().file_type, FileType::Regular);
        assert_eq!(
            symlink_metadata(b"/sym").unwrap().file_type,
            FileType::Symlink
        );
        assert_eq!(read_link(b"/sym").unwrap(), b"/hard");
        rename(b"/sym", b"/a/sym").unwrap();
        assert_eq!(read_link(b"/a/sym").unwrap(), b"/hard");

        let mut hard = open(
            b"/hard",
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

        let entries = read_dir(b"/b").unwrap();
        assert!(entries.iter().all(|entry| entry.file_id != 0));

        assert!(entries.iter().any(|entry| entry.name == b"file"));

        unlink(b"/b/file").unwrap();
        assert_eq!(metadata(b"/hard").unwrap().size, 2);

        mkdir(b"/old", FilePermissions::DEFAULT_DIRECTORY).unwrap();
        rename(b"/old", b"/new").unwrap();
        mkdir(b"/a/dir", FilePermissions::DEFAULT_DIRECTORY).unwrap();

        assert_eq!(rename(b"/a/dir", b"/b/dir"), Err(VfsError::Unsupported));

        rmdir(b"/a/dir").unwrap();
        rmdir(b"/new").unwrap();
        unlink(b"/a/sym").unwrap();
        unlink(b"/hard").unwrap();
        rmdir(b"/a").unwrap();
        rmdir(b"/b").unwrap();

        sync().unwrap();
    }
);

roxy_test::kernel_test!("roxy-ext4::open-no-follow", refuses_trailing_symlink, {
    let options = OpenOptions {
        access: OpenAccess::ReadWrite,
        creation: CreationMode::CreateNew,
        permissions: FilePermissions::new(0o640).unwrap(),
        append: false,
        truncate: false,
        no_follow: false,
    };
    open(b"/target", options).unwrap();
    symlink(b"/target", b"/link").unwrap();

    // Without O_NOFOLLOW the trailing symlink is followed to the regular file.
    let followed = open(
        b"/link",
        OpenOptions {
            access: OpenAccess::ReadOnly,
            no_follow: false,
            ..OpenOptions::read_only()
        },
    )
    .unwrap();
    assert_eq!(followed.metadata().unwrap().file_type, FileType::Regular);

    // With O_NOFOLLOW the trailing symlink is rejected with ELOOP.
    match open(
        b"/link",
        OpenOptions {
            access: OpenAccess::ReadOnly,
            no_follow: true,
            ..OpenOptions::read_only()
        },
    ) {
        Err(VfsError::Loop) => {}
        Err(other) => panic!("expected Err(Loop), got {other:?}"),
        Ok(_) => panic!("expected Err(Loop), got Ok"),
    }
    unlink(b"/link").unwrap();
    unlink(b"/target").unwrap();
});
