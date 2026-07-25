#!/usr/bin/env bash

autotools_configure() {
	local configure_script="${autotools_configure_script:-${source_dir}/configure}"

	export AR=llvm-ar
	export CC="clang --target=x86_64-unknown-roxy --sysroot=/sysroot"
	export CC_FOR_BUILD=clang
	export NM=llvm-nm
	export RANLIB=llvm-ranlib
	export STRIP=llvm-strip

	"${configure_script}" \
		--build="$(clang -dumpmachine)" \
		--host=x86_64-unknown-none \
		--prefix="${prefix}" \
		"$@"
}

autotools_build() {
	make -j "${parallelism}"
}

autotools_install() {
	make DESTDIR="${dest_dir}" install
}
