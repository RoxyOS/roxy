#!/usr/bin/env bash

autotools_patch_roxy_target() {
	local config_sub="${base_dir}/toolchains/config.sub"
	local configure_script="${autotools_configure_script:-${source_dir}/configure}"

	local f
	for f in $(find "${source_dir}" -name config.sub -type f); do
		if grep -q 'GNU config.sub' "${f}" && ! grep -q 'roxy-mlibc' "${f}"; then
			cp "${config_sub}" "${f}"
		fi
	done

	if ! grep -q 'roxy-mlibc' "${configure_script}" 2>/dev/null \
		&& grep -q '\*-mlibc)' "${configure_script}" 2>/dev/null; then
		return 0
	fi
	if ! grep -q 'roxy-mlibc' "${configure_script}" 2>/dev/null \
		&& grep -q 'kopensolaris\*-gnu' "${configure_script}" 2>/dev/null; then
		sed -i 's/\(kopensolaris\*-gnu\)/\1 | roxy-mlibc/g' "${configure_script}"
	fi
}

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
		--host=x86_64-unknown-roxy-mlibc \
		--prefix="${prefix}" \
		"$@"
}

autotools_build() {
	make -j "${parallelism}"
}

autotools_install() {
	make DESTDIR="${dest_dir}" install
}
