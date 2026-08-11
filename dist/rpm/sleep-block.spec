# Binary package: the Makefile compiles and tests with the local toolchain, and
# rpmbuild only stages the finished artefacts. So there is no %build step and no
# BuildRequires on rust/cargo -- which also means this spec works with a rustup
# toolchain that RPM knows nothing about.
#
# The source tarball is laid out by `make package` and mirrors the final install
# layout: the binary, the desktop entry, and an icons/ directory.

# The release profile strips symbols and applies fat LTO, so there is nothing
# useful left for debuginfo extraction to find.
%global debug_package %{nil}

# The binary is already built and stripped; skip the post-processing that would
# otherwise try to re-strip it or generate build-ids from missing symbols.
%global __brp_strip %{nil}
%global __brp_strip_static_archive %{nil}

# The staged tarball already contains a binary built for a specific
# architecture, which is not necessarily the build host's. `make package` passes
# `--target` so rpmbuild tags the package correctly; without it an aarch64
# binary would be packaged and labelled x86_64.
# Cargo.toml is the single source of truth for the version. `make package`
# passes it as --define 'version ...'; the fallback below only applies when the
# spec is built by hand, and is deliberately obvious rather than plausible so a
# mismatched package is easy to spot.
%{!?version: %global version 0.0.0}

Name:           sleep-block
Version:        %{version}
Release:        1%{?dist}
Summary:        GUI toggle that stops the system from sleeping

License:        MIT
URL:            https://github.com/dweiner/sleep-block
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  desktop-file-utils

# systemd-logind provides the sleep inhibitor interface the app calls. Without
# it the app starts but can never acquire a lock.
Requires:       systemd
Requires:       hicolor-icon-theme

# The binary links the GPU and font stack statically, but still dlopen()s the
# platform libraries at runtime.
Requires:       libxkbcommon

%description
A small GUI utility that blocks system suspend through systemd-logind, the same
mechanism systemd-inhibit(1) uses. Click the indicator to toggle: green means
the system is being kept awake, red means it may suspend normally.

An optional setting additionally blocks screen blanking and locking through the
org.freedesktop.ScreenSaver interface, which is a separate mechanism from sleep
and is left off by default.

A tray icon reflects the current state and can toggle it without opening the
window.

%prep
%autosetup -n %{name}-%{version}

%install
install -Dpm0755 %{name} %{buildroot}%{_bindir}/%{name}

desktop-file-install \
    --dir=%{buildroot}%{_datadir}/applications \
    %{name}.desktop

# The binary embeds its own tray icons, but the desktop entry resolves its icon
# through the theme, so the PNGs are installed as well.
for size in 16 22 24 32 48 64 128 256; do
    install -Dpm0644 icons/%{name}-active-${size}.png \
        %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/%{name}.png
done
install -Dpm0644 icons/%{name}-active.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/%{name}.svg

install -Dpm0644 LICENSE %{buildroot}%{_datadir}/licenses/%{name}/LICENSE
install -Dpm0644 README.md %{buildroot}%{_datadir}/doc/%{name}/README.md

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/%{name}.desktop

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/*/apps/%{name}.png
%{_datadir}/icons/hicolor/scalable/apps/%{name}.svg

%changelog
* Mon Aug 10 2026 Daniel Weiner <info@phantomnet.net> - %{version}-1
- Built from Cargo.toml version %{version}; see the git log and tags for the
  change history rather than maintaining it in two places.
