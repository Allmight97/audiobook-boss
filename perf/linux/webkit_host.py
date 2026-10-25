# TEMPORARY: a bare WebKitGTK window that loads the harness, like Tauri's webview. Removed before merge.
import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2

window = Gtk.Window()
window.set_default_size(1440, 900)
view = WebKit2.WebView()
window.add(view)
view.connect('notify::title', lambda v, _p: print(f'title: {v.get_title()}', flush=True))
view.load_uri(sys.argv[1])
window.connect('destroy', Gtk.main_quit)
window.show_all()
Gtk.main()
