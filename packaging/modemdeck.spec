Name:           modemdeck
Version:        2.0.0
Release:        1%{?dist}
Summary:        Universal WWAN and Mobile Broadband Management Suite by MilMit
License:        GPL-3.0-or-later
URL:            https://milmit.net
Source0:        https://github.com/MilMit/modemdeck-linux/archive/refs/tags/v%{version}.tar.gz
Requires:       ModemManager, NetworkManager, fontconfig, libxkbcommon
BuildRequires:  cargo, rust, fontconfig-devel, libxkbcommon-devel, libudev-devel

%description
ModemDeck is a modern Rust + Slint cellular modem manager and network analyzer.

%prep
%autosetup -n modemdeck-linux-%{version}

%build
cargo build --release --locked

%check
cargo test --locked --all-targets

%install
install -Dm755 target/release/modemdeck %{buildroot}%{_bindir}/modemdeck
install -Dm644 data/net.milmit.ModemDeck.desktop %{buildroot}%{_datadir}/applications/net.milmit.ModemDeck.desktop
install -Dm644 data/net.milmit.ModemDeck.metainfo.xml %{buildroot}%{_datadir}/metainfo/net.milmit.ModemDeck.metainfo.xml
install -Dm644 assets/icons/net.milmit.ModemDeck.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg
install -Dm644 data/net.milmit.ModemDeck-monitor.service %{buildroot}%{_userunitdir}/net.milmit.ModemDeck-monitor.service
install -Dm644 LICENSE %{buildroot}%{_licensedir}/%{name}/LICENSE

%files
%{_bindir}/modemdeck
%{_datadir}/applications/net.milmit.ModemDeck.desktop
%{_datadir}/metainfo/net.milmit.ModemDeck.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/net.milmit.ModemDeck.svg
%{_userunitdir}/net.milmit.ModemDeck-monitor.service
%license %{_licensedir}/%{name}/LICENSE
