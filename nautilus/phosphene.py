import os
import subprocess
from gi.repository import Nautilus, GObject

class PhospheneExtension(GObject.GObject, Nautilus.MenuProvider):
    def __init__(self):
        pass

    def get_file_items(self, *args):
        # Supports Nautilus 3 and 4 API changes
        if len(args) == 2:
            window, files = args
        elif len(args) == 1:
            files = args[0]
        else:
            return []

        if len(files) != 1:
            return []

        file = files[0]
        if file.is_directory() or file.get_uri_scheme() != 'file':
            return []

        item = Nautilus.MenuItem(
            name="Phosphene::Triage",
            label="Triage with Phosphene",
            tip="Visualize binary entropy and check modifications",
            icon="view-preview"
        )

        filepath = file.get_location().get_path()
        item.connect("activate", self.menu_activate_cb, filepath)

        return [item]

    def menu_activate_cb(self, menu, filepath):
        # We redirect stdout/stderr to /dev/null so Nautilus doesn't hang.
        # Phosphene will detect it is NOT in a TTY and launch its Wayland-native GUI mode.
        subprocess.Popen(['phosphene', filepath], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
