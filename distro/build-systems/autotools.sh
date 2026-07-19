#!/usr/bin/env bash

autotools_configure() {
	export AR=llvm-ar
	export CC="${base_dir}/toolchains/x86_64-roxy-clang"
	export CC_FOR_BUILD=clang
	export NM=llvm-nm
	export RANLIB=llvm-ranlib
	export STRIP=llvm-strip

	"${source_dir}/configure" \
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
