#!/usr/bin/env bash

meson_configure() {
	meson setup \
		. \
		"${source_dir}" \
		--cross-file="${base_dir}/toolchains/x86_64-roxy.cross-file" \
		--prefix="${prefix}" \
		--libdir=lib \
		--localstatedir=/var \
		"$@"
}

meson_build() {
	meson compile -C . -j "${parallelism}"
}

meson_install() {
	DESTDIR="${dest_dir}" meson install -C . --no-rebuild
}
