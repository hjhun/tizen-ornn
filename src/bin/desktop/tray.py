import os
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("AyatanaAppIndicator3", "0.1")
from gi.repository import AyatanaAppIndicator3 as Indicator, GLib, Gtk

parent = int(os.environ["ORNN_TRAY_PARENT"])
name = os.environ["ORNN_TRAY_IMAGE"]
menu = Gtk.Menu()
header = Gtk.MenuItem(label=f"{name} ({os.environ['ORNN_TRAY_RUNNING']})")
header.set_sensitive(False)
menu.append(header)
for title, action in ((os.environ["ORNN_TRAY_TERMINAL"], "terminal"), (os.environ["ORNN_TRAY_SETTINGS"], "settings"),
                      (os.environ["ORNN_TRAY_SHUTDOWN"], "shutdown")):
    item = Gtk.MenuItem(label=title)
    item.connect("activate", lambda _, cmd=action: print(cmd, flush=True))
    menu.append(item)
menu.show_all()
icon = Indicator.Indicator.new(f"tizen-ornn-{parent}", "computer",
                               Indicator.IndicatorCategory.APPLICATION_STATUS)
icon.set_title(f"Tizen Ornn - {name}")
icon.set_menu(menu)
icon.set_status(Indicator.IndicatorStatus.ACTIVE)


def check_parent():
    if os.getppid() == parent:
        return True
    Gtk.main_quit()
    return False


GLib.timeout_add(500, check_parent)
Gtk.main()
icon.set_status(Indicator.IndicatorStatus.PASSIVE)
