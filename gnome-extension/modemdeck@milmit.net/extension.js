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
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addAction('Open ModemDeck', () => this._openManager());
        this.menu.addAction('Refresh status', () => this._reload());
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._brandItem = this.menu.addAction('MilMit • milmit.net', () => {});
        this._brandItem.setSensitive(false);

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

    _reload() {
        try {
            const [ok, bytes] = this._file.load_contents(null);
            if (!ok)
                return;
            const data = JSON.parse(new TextDecoder().decode(bytes));
            const network = data.network || '—';
            const signal = Number(data.signal_percent || 0);
            const band = data.band ? ` • ${data.band}` : '';
            this._label.text = network;
            this._icon.icon_name = this._signalIcon(signal);
            this._statusItem.label.text = `${data.operator || 'No operator'} • ${network}${band} • ${signal}%`;
        } catch (_error) {
            this._label.text = '—';
            this._icon.icon_name = 'network-cellular-signal-none-symbolic';
            this._statusItem.label.text = 'ModemDeck monitor is not running';
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
