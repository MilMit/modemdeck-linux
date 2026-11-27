import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

class ModemDeckIndicator extends PanelMenu.Button {
    constructor() {
        super(0.0, 'ModemDeck');

        this._box = new St.BoxLayout({style_class: 'panel-status-menu-box'});
        this._icon = new St.Icon({
            icon_name: 'network-cellular-signal-none-symbolic',
            style_class: 'system-status-icon',
        });
        this._label = new St.Label({
            text: '—',
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._box.add_child(this._icon);
        this._box.add_child(this._label);
        this.add_child(this._box);

        this._statusItem = this.menu.addAction('Waiting for ModemDeck monitor…', () => {});
        this._statusItem.setSensitive(false);
        this._rfItem = this.menu.addAction('RF: —', () => {});
        this._rfItem.setSensitive(false);
        this._trafficItem = this.menu.addAction('Traffic: — • Health: —', () => {});
        this._trafficItem.setSensitive(false);
        this._smsItem = this.menu.addAction('Unread SMS: 0 • Lock: Auto', () => {});
        this._smsItem.setSensitive(false);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addAction('Open ModemDeck', () => this._openManager());
        this.menu.addAction('Connect Mobile Broadband', () => this._nmAction(['nmcli', 'radio', 'wwan', 'on']));
        this.menu.addAction('Disconnect Mobile Broadband', () => this._nmAction(['nmcli', 'radio', 'wwan', 'off']));
        this.menu.addAction('Export Diagnostics', () => this._runCli(['diagnostics', 'export']));
        this.menu.addAction('Refresh status', () => this._reload());
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._brandItem = this.menu.addAction('MilMit • milmit.net', () => {});
        this._brandItem.setSensitive(false);
        this._applyLocale();

        const runtime = GLib.getenv('XDG_RUNTIME_DIR') ?? GLib.build_filenamev([GLib.get_tmp_dir(), 'modemdeck-runtime']);
        this._file = Gio.File.new_for_path(GLib.build_filenamev([runtime, 'modemdeck-status.json']));
        try {
            this._monitor = this._file.monitor_file(Gio.FileMonitorFlags.NONE, null);
            this._monitor.connect('changed', () => {
                GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                    this._reload();
                    return GLib.SOURCE_REMOVE;
                });
            });
        } catch (error) {
            logError(error, 'ModemDeck: could not monitor runtime status file');
        }
        this._reload();
    }

    _applyLocale() {
        try {
            const cfgPath = GLib.build_filenamev([GLib.get_user_config_dir(), 'modemdeck', 'settings.json']);
            const cfgFile = Gio.File.new_for_path(cfgPath);
            if (!cfgFile.query_exists(null)) return;
            const [ok, bytes] = cfgFile.load_contents(null);
            if (!ok) return;
            const settings = JSON.parse(new TextDecoder().decode(bytes));
            const lang = (settings.language || 'en').toLowerCase();
            const map = {
                fa: { open: 'باز کردن ModemDeck', conn: 'اتصال اینترنت همراه', disc: 'قطع اینترنت همراه', diag: 'خروجی عیب‌یابی', ref: 'بروزرسانی وضعیت' },
                ar: { open: 'فتح ModemDeck', conn: 'توصيل الإنترنت الخلوي', disc: 'فصل الإنترنت الخلوي', diag: 'تصدير التشخيص', ref: 'تحديث الحالة' },
                tr: { open: "ModemDeck'i Aç", conn: 'Mobil Genişbandı Bağla', disc: 'Bağlantıyı Kes', diag: 'Tanılama Dışa Aktar', ref: 'Durumu Yenile' },
                de: { open: 'ModemDeck öffnen', conn: 'Mobilfunk verbinden', disc: 'Mobilfunk trennen', diag: 'Diagnose exportieren', ref: 'Status aktualisieren' },
                fr: { open: 'Ouvrir ModemDeck', conn: 'Connecter haut débit mobile', disc: 'Déconnecter', diag: 'Exporter diagnostic', ref: 'Actualiser' },
                es: { open: 'Abrir ModemDeck', conn: 'Conectar banda ancha móvil', disc: 'Desconectar', diag: 'Exportar diagnóstico', ref: 'Actualizar estado' },
                ru: { open: 'Открыть ModemDeck', conn: 'Подключить мобильную сеть', disc: 'Отключить сеть', diag: 'Экспорт диагностики', ref: 'Обновить статус' },
            };
            const tr = map[lang];
            if (!tr) return;
            const items = this.menu._getMenuItems();
            for (const it of items) {
                if (!it.label) continue;
                const txt = it.label.text;
                if (txt === 'Open ModemDeck') it.label.text = tr.open;
                else if (txt === 'Connect Mobile Broadband') it.label.text = tr.conn;
                else if (txt === 'Disconnect Mobile Broadband') it.label.text = tr.disc;
                else if (txt === 'Export Diagnostics') it.label.text = tr.diag;
                else if (txt === 'Refresh status') it.label.text = tr.ref;
            }
        } catch (_e) {}
    }

    _reload() {
        try {
            const [ok, bytes] = this._file.load_contents(null);
            if (!ok)
                return;
            const data = JSON.parse(new TextDecoder().decode(bytes));
            const network = data.network || '—';
            const signal = Number(data.signal_percent || 0);
            const band = data.band ? ` • ${data.band}` : '';
            const ca = data.carrier_aggregation ? ' • CA' : '';
            this._label.text = `${network}${ca}`;
            this._icon.icon_name = this._signalIcon(signal);
            this._statusItem.label.text = `${data.operator || 'No operator'} • ${network}${band}${ca} • ${signal}%`;
            const rsrp = data.rsrp != null ? `${data.rsrp} dBm` : '—';
            const rsrq = data.rsrq != null ? `${data.rsrq} dB` : '—';
            const sinr = data.sinr != null ? `${data.sinr} dB` : '—';
            this._rfItem.label.text = `RSRP ${rsrp} • RSRQ ${rsrq} • SINR ${sinr}`;
            const rxKbps = Math.round(Number(data.rx_rate_bps || 0) / 1024);
            const txKbps = Math.round(Number(data.tx_rate_bps || 0) / 1024);
            const usageMb = Math.round(Number(data.data_usage_bytes || 0) / (1024 * 1024));
            this._trafficItem.label.text = `↓${rxKbps} ↑${txKbps} KB/s • Used: ${usageMb} MB • ${data.internet_health || 'Unknown'}`;
            this._smsItem.label.text = `Unread SMS: ${Number(data.unread_sms || 0)} • Lock: ${data.lock_mode || 'Auto'}`;
        } catch (_error) {
            this._label.text = '—';
            this._icon.icon_name = 'network-cellular-signal-none-symbolic';
            this._statusItem.label.text = 'ModemDeck monitor is not running';
            this._rfItem.label.text = 'RF: —';
            this._trafficItem.label.text = 'Traffic: —';
            this._smsItem.label.text = 'Unread SMS: 0';
        }
    }

    _runCli(args) {
        try {
            Gio.Subprocess.new(['modemdeck', ...args], Gio.SubprocessFlags.NONE);
        } catch (error) {
            logError(error, 'ModemDeck: could not run CLI action');
        }
    }

    _nmAction(argv) {
        try {
            Gio.Subprocess.new(argv, Gio.SubprocessFlags.NONE);
        } catch (error) {
            logError(error, 'ModemDeck: could not run NetworkManager quick action');
        }
    }

    _signalIcon(signal) {
        if (signal >= 75)
            return 'network-cellular-signal-excellent-symbolic';
        if (signal >= 50)
            return 'network-cellular-signal-good-symbolic';
        if (signal >= 25)
            return 'network-cellular-signal-ok-symbolic';
        if (signal > 0)
            return 'network-cellular-signal-weak-symbolic';
        return 'network-cellular-signal-none-symbolic';
    }

    _openManager() {
        try {
            const appInfo = Gio.DesktopAppInfo.new('net.milmit.ModemDeck.desktop');
            if (appInfo) {
                appInfo.launch([], null);
                return;
            }
            Gio.Subprocess.new(['modemdeck'], Gio.SubprocessFlags.NONE);
        } catch (error) {
            logError(error, 'ModemDeck: could not launch manager');
        }
    }

    destroy() {
        if (this._monitor) {
            this._monitor.cancel();
            this._monitor = null;
        }
        super.destroy();
    }
}

export default class ModemDeckExtension extends Extension {
    enable() {
        this._indicator = new ModemDeckIndicator();
        Main.panel.addToStatusArea('modemdeck', this._indicator);
    }

    disable() {
        this._indicator?.destroy();
        this._indicator = null;
    }
}
