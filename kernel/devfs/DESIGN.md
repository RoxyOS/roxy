# `devfs` Design

## Purpose and scope

`roxy-devfs` provides the kernel's device filesystem: a `Device` trait for character devices, a
path-scoped device registry, and a read-only `FileSystem` implementation mounted at `/dev`. It
owns name lookup and descriptor adaptation only; device semantics, state, and hardware ownership
remain in driver subsystems such as `roxy-fbdev`.

The null sink pseudo-device (`/dev/null`) is an exception: it has no hardware, driver, or state,
so it is implemented as a built-in device within this crate. The composition root registers it
unconditionally alongside hardware drivers. Block devices, fifos, and sockets are out of scope;
`Device` currently describes character devices only.

## Ownership model

`DeviceRegistry` owns a `BTreeMap` from mount-relative byte paths (such as `fb0`) to
`Arc<dyn Device>`. Registration happens exactly once during kernel initialization and never
removes entries; the registry and its devices therefore live for the kernel lifetime. The
composition root (`kernel-main`) creates one registry, hands it to `DevFs` for the `/dev` mount,
and passes the same `Arc` to drivers that register their devices.

The null sink (`NullDevice`) is an intrinsic pseudo-device defined in this crate. The composition
root calls `register_null` to register it under `null`; unlike `fb0` this registration is
unconditional because `/dev/null` must always exist regardless of hardware.

`DevFs` implements `FileSystem` against that registry. Its mount root is a directory whose
entries are the registered device names; every other path either resolves to a registered device
or fails with `NotFound`. `DeviceFile` adapts one `Arc<dyn Device>` to the VFS `FileHandle`
contract, forwarding ioctl and mmap requests and rejecting read, write, seek, and truncate with
the owning device's policy. Metadata is reported by the device itself so file IDs stay stable
across open and directory listing.

## Contract

```text
open("/dev/fb0")
  → VFS mount routing → DevFs::open → registry lookup → DeviceFile { device }
  → VfsFile → descriptor-layer File

open("/dev/null")
  → (same path, NullDevice)
```

- `Device::metadata` returns the character-device metadata including a stable per-device file ID.
  `NullDevice` reports file ID 2, character-device type, mode 0666, and zero size.
- `Device::ioctl` receives the same typed `IoctlRequest` the descriptor layer dispatches; the
  device returns `IoctlError` values that the syscall layer maps to errno.
- `Device::mmap(size, offset)` describes the device's physical memory for a file-backed `mmap`;
  the mapping itself is installed by the VM layer, never by the device.
- `Device::poll` defaults to immediate readiness; stream devices override it.
- `Device::read`/`write` default to `BadOperation`; framebuffer-style devices keep the default.
  `NullDevice` overrides: `read` returns EOF (zero bytes) and `write` accepts-and-discards all
  input, reporting the full length as successful.

## Invariants and limits

- Registration is idempotent-failing: a second registration under an existing path returns
  `AlreadyExists` and never replaces the existing device.
- `DevFs` rejects all namespace mutations with `ReadOnly`, so the mount never observes
  unregistered nodes and active-handle tracking cannot race device removal.
- The mount root must exist as a logical path even though the root filesystem may not contain a
  `/dev` directory; VFS mount routing resolves `/dev`-prefixed paths before the root filesystem
  sees them.
- Device read/write errors map to VFS errors at the adapter boundary; ioctl and mmap errors pass
  through unchanged because they already use descriptor-layer types.
- There is no device enumeration, hotplug, or devtmpfs-style node persistence; the device set is
  fixed at boot.
