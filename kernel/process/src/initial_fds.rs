use roxy_fd::FdTable;
use spin::Once;

/// Populates the initially empty descriptor table of a directly spawned process.
pub type InitialFdInjector = fn(&mut FdTable);

static INITIAL_FD_INJECTOR: Once<InitialFdInjector> = Once::new();

pub(super) fn register(injector: InitialFdInjector) {
    assert!(
        INITIAL_FD_INJECTOR.get().is_none(),
        "initial FD injector was already registered"
    );
    INITIAL_FD_INJECTOR.call_once(|| injector);
}

pub(super) fn inject(table: &mut FdTable) {
    let injector = INITIAL_FD_INJECTOR
        .get()
        .expect("initial FD injector must be registered before spawning processes");

    injector(table);
}
