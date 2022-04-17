#!/usr/bin/python3

import string
import subprocess
import sys
import random

from argparse import ArgumentParser, SUPPRESS
from gi.repository import GLib, Gio


PREFIX = "com.canonical.Terminal."


class GnomeTerminal(object):
    @staticmethod
    def generate_random_string(length=32,
                               chars=string.ascii_lowercase +
                               string.ascii_uppercase):
        return ''.join(random.choice(chars) for _ in range(length))

    @staticmethod
    def find_new_name():
        name = PREFIX + GnomeTerminal.generate_random_string()
        proxy = Gio.DBusProxy.new_for_bus_sync(
            Gio.BusType.SESSION,
            Gio.DBusProxyFlags.DO_NOT_AUTO_START |
            Gio.DBusProxyFlags.DO_NOT_CONNECT_SIGNALS |
            Gio.DBusProxyFlags.DO_NOT_LOAD_PROPERTIES,
            None,
            name,
            "/org/gnome/Terminal",
            "org.freedesktop.Application",
            None)
        return name if proxy.get_name_owner() is None else find_new_name()

    def exit_loop(self, a, b, subprocess):
        sys.exit(subprocess.get_exit_status())

    def server_appeared(self, con, name, owner):
        # start gnome-terminal now
         Gio.Subprocess.new(['/usr/bin/gnome-terminal.real',
                             '--app-id', name] +
                             self.args,
                             Gio.SubprocessFlags.NONE)

    def spawn_terminal_server(self, name, cls):
        args = ['/usr/libexec/gnome-terminal-server',
                '--app-id',
                 name]
        if cls is not None:
            args.extend(['--class', cls])
        ts = Gio.Subprocess.new(args, Gio.SubprocessFlags.NONE)
        ts.wait_async(None, self.exit_loop, ts)

    def __init__(self, args, mainloop):
        self.name = None
        self.mainloop = mainloop

        parser = ArgumentParser(add_help=False,  usage=SUPPRESS)
        parser.add_argument('--app-id', action='store', dest='appid')

        # swallow these arguments
        parser.add_argument('-h', '--help', action='store_true', dest='help')
        parser.add_argument('--disable-factory', action='store_true')
        parser.add_argument('--class', action='store', dest='cls')

        cmdargs, unknown = parser.parse_known_args()

        self.args = unknown
        self.args.insert(0, "/usr/bin/gnome-terminal.real")

        if cmdargs.help:
            self.args.append('--help')

        if cmdargs.cls is not None and cmdargs.appid is None:
            self.name = PREFIX + cmdargs.cls
        elif cmdargs.appid is not None:
            self.name = cmdargs.appid

        # --disable-factory, --class and --app-id weren't supplied, so just
        # invoke g-t
        if not cmdargs.disable_factory and self.name is None:
            sys.exit(subprocess.call(self.args))

        if self.name is None:
            self.name = self.find_new_name()

        Gio.bus_watch_name(Gio.BusType.SESSION,
                           self.name,
                           Gio.BusNameWatcherFlags.NONE,
                           self.server_appeared,
                           None)

        self.spawn_terminal_server(self.name, cmdargs.cls)


def main():
    mainloop = GLib.MainLoop()
    GnomeTerminal(sys.argv[:], mainloop)

    try:
        mainloop.run()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
