ENTRY(_start)

PHDRS {
    requests PT_LOAD FLAGS(4);
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS {
    . = 0xffffffff80000000;
    .limine_requests : {
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_end))
    } :requests
    . = ALIGN(4096);
    .text : { *(.text .text.*) } :text
    . = ALIGN(4096);
    .rodata : { *(.rodata .rodata.*) } :rodata
    linkme_TESTS : {
        __start_linkme_TESTS = .;
        KEEP(*(linkme_TESTS))
        __stop_linkme_TESTS = .;
    } :rodata
    linkm2_TESTS : {
        __start_linkm2_TESTS = .;
        KEEP(*(linkm2_TESTS))
        __stop_linkm2_TESTS = .;
    } :rodata
    . = ALIGN(4096);
    .data : { *(.data .data.*) } :data
    .bss : {
        __bss_start = .;
        *(.bss .bss.* COMMON)
        __bss_end = .;
    } :data
    . = ALIGN(4096);
    __kernel_end = .;
    /DISCARD/ : { *(.eh_frame*) *(.note*) }
}
