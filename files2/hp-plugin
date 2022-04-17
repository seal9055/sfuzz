#!/usr/bin/python3
# -*- coding: utf-8 -*-
#
# (c) Copyright 2003-2015 HP Development Company, L.P.
#
# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program; if not, write to the Free Software
# Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307 USA
#
# Author: Don Welch
#

__version__ = '2.1'
__mod__ = 'hp-plugin'
__title__ = 'Plugin Download and Install Utility'
__doc__ = "HP Proprietary Plugin Download and Install Utility"

# Std Lib
import sys
import getopt
import time
import os.path
import re
import os
import gzip


# Local
from base.g import *
from base.strings import *
from base import device, utils, tui, module, services
from base.sixext.moves import input
from prnt import cups

try:
    from importlib import import_module
except ImportError as e:
    log.debug(e)
    from base.utils import dyn_import_mod as import_module


pm = None

def plugin_download_callback(c, s, t):
    pm.update(int(100*c*s/t),
             utils.format_bytes(c*s))


def plugin_install_callback(s):
    print(s)

def clean_exit(code=0):
    mod.unlockInstance()
    sys.exit(code)

USAGE = [ (__doc__, "", "name", True),
          ("Usage: %s [MODE] [OPTIONS]" % __mod__, "", "summary", True),
          utils.USAGE_MODE,
          utils.USAGE_GUI_MODE,
          utils.USAGE_INTERACTIVE_MODE,
          ("Installation for required printer mode:", "--required (Qt4 only)", "option", False),
          ("Installation for optional printer mode:", "--optional (Qt4 only)", "option", False),
          #("Installation generic mode:", "--generic (default)", "option", False),
          utils.USAGE_LANGUAGE,
          utils.USAGE_OPTIONS,
          ("Specify the path to the plugin file:", "-p <path> or --path=<path> or --plugin=<path>", "option", False),
          utils.USAGE_LOGGING1, utils.USAGE_LOGGING2, utils.USAGE_LOGGING3,
          utils.USAGE_HELP,
          utils.USAGE_SPACE,
          utils.USAGE_SEEALSO,
          ("hp-setup", "", "seealso", False),
          ("hp-firmware", "", "seealso", False),
        ]


mod = module.Module(__mod__, __title__, __version__, __doc__, USAGE,
                    (INTERACTIVE_MODE, GUI_MODE),
                    (UI_TOOLKIT_QT3, UI_TOOLKIT_QT4, UI_TOOLKIT_QT5), True)

opts, device_uri, printer_name, mode, ui_toolkit, loc = \
    mod.parseStdOpts('sp:', ['path=', 'plugin=', 'plug-in=', 'reason=',
                            'generic', 'optional', 'required'],
                     handle_device_printer=False)

plugin_path = None
install_mode = PLUGIN_NONE # reuse plugin types for mode (PLUGIN_NONE = generic)
plugin_reason = PLUGIN_REASON_NONE
Is_quiet_mode = False
for o, a in opts:
    if o in ('-p', '--path', '--plugin', '--plug-in'):
        plugin_path = os.path.normpath(os.path.abspath(os.path.expanduser(a)))

    elif o == '--required':
        install_mode = PLUGIN_REQUIRED
        if ui_toolkit == 'qt3':
            log.warn("--required switch ignored.")

    elif o == '--optional':
        install_mode = PLUGIN_OPTIONAL
        if ui_toolkit == 'qt3':
            log.warn("--optional switch ignored.")

    elif o == '--reason':
        plugin_reason = int(a)
        
    elif o == '-s':
        Is_quiet_mode = True

if services.running_as_root():
    log.warn("It is not recommended to run 'hp-plugin' in a root mode.")
    mode = INTERACTIVE_MODE
    #sys.exit(1)

if not Is_quiet_mode:
    mod.quiet= False
    mod.showTitle()
    
version = prop.installed_version
plugin_filename = 'hplip-%s-plugin.run' % version

ok= mod.lockInstance()
if ok is False:
    log.error("Plug-in lock acquire failed. check if hp-plugin is already running")
    sys.exit(1)

if plugin_path is not None:
    if not os.path.exists(plugin_path):
        log.error("Plug-in path '%s' not found." % plugin_path)
        clean_exit(1)

    if os.path.isdir(plugin_path):
        plugin_path = os.path.join(plugin_path, 'hplip-%s-plugin.run' % version)

        if not os.path.exists(plugin_path):
            log.error("Plug-in path '%s' not found." % plugin_path)
            clean_exit(1)

    if os.path.basename(plugin_path) != plugin_filename:
        log.error("Plug-in filename must be '%s'." % plugin_filename)
        clean_exit(1)


    size, checksum, timestamp = os.stat(plugin_path)[6], '', 0.0
    plugin_path = 'file://' + plugin_path
    log.debug("Plugin path=%s (%d)" % (plugin_path, size))


if mode == GUI_MODE:
    if ui_toolkit == 'qt3':
        if not utils.canEnterGUIMode():
            log.error("%s requires GUI support (try running with --qt4). Try using interactive (-i) mode." % __mod__)
            clean_exit(1)
    else:
        if not utils.canEnterGUIMode4():
            log.error("%s requires GUI support (try running with --qt3). Try using interactive (-i) mode." % __mod__)
            clean_exit(1)


PKIT = utils.to_bool(sys_conf.get('configure', 'policy-kit'))
if PKIT:
    try:
        from base.pkit import *
        try:
            pkit = PolicyKit()
            pkit_installed = True
        except dbus.DBusException as ex:
            log.error("PolicyKit support requires DBUS or PolicyKit support files missing")
            pkit_installed = False
    except:
        log.error("Unable to load pkit...is HPLIP installed?")
        pkit_installed = False
else:
    pkit_installed = False

from installer import pluginhandler
pluginObj = pluginhandler.PluginHandle()
plugin_installed = False
if pluginObj.getStatus() == pluginhandler.PLUGIN_INSTALLED and plugin_path is None:
    plugin_installed = True
if mode == GUI_MODE:
    if ui_toolkit == 'qt3':
        try:
            from qt import *
            from ui import pluginform2
        except ImportError:
            log.error("Unable to load Qt3 support. Is it installed?")
            clean_exit(1)

        app = QApplication(sys.argv)
        QObject.connect(app, SIGNAL("lastWindowClosed()"), app, SLOT("quit()"))

        if loc is None:
            loc = user_conf.get('ui', 'loc', 'system')
            if loc.lower() == 'system':
                loc = str(QTextCodec.locale())
                log.debug("Using system locale: %s" % loc)

        if loc.lower() != 'c':
            e = 'utf8'
            try:
                l, x = loc.split('.')
                loc = '.'.join([l, e])
            except ValueError:
                l = loc
                loc = '.'.join([loc, e])

            log.debug("Trying to load .qm file for %s locale." % loc)
            trans = QTranslator(None)

            qm_file = 'hplip_%s.qm' % l
            log.debug("Name of .qm file: %s" % qm_file)
            loaded = trans.load(qm_file, prop.localization_dir)

            if loaded:
                app.installTranslator(trans)
            else:
                loc = 'c'

        if loc == 'c':
            log.debug("Using default 'C' locale")
        else:
            log.debug("Using locale: %s" % loc)
            QLocale.setDefault(QLocale(loc))
            prop.locale = loc
            try:
                locale.setlocale(locale.LC_ALL, locale.normalize(loc))
            except locale.Error:
                pass
        
        w = pluginform2.PluginForm2()
        app.setMainWidget(w)
        w.show()

        app.exec_loop()

    else: # qt4
        # try:
        #     from PyQt4.QtGui import QApplication, QMessageBox
        #     from ui4.plugindialog import PluginDialog
        # except ImportError:
        #     log.error("Unable to load Qt4 support. Is it installed?")
        #     clean_exit(1)

        QApplication, ui_package = utils.import_dialog(ui_toolkit)
        ui = import_module(ui_package + ".plugindialog")
        if ui_toolkit == "qt5":
            from PyQt5.QtWidgets import QMessageBox
        elif ui_toolkit == "qt4":
            from PyQt4.QtGui import QMessageBox
        app = QApplication(sys.argv)
        if plugin_installed:
            if QMessageBox.question(None,
                                 " ",
                                 "The driver plugin for HPLIP %s appears to already be installed. Do you wish to download and re-install the plug-in?"%version,
                                  QMessageBox.Yes | QMessageBox.No) != QMessageBox.Yes:
                clean_exit(1)

        dialog = ui.PluginDialog(None, install_mode, plugin_reason)
        dialog.show()
        try:
            log.debug("Starting GUI loop...")
            app.exec_()
        except KeyboardInterrupt:
            log.error("User exit")
            clean_exit(0)


else: # INTERACTIVE_MODE
    try:
        
        log.info("(Note: Defaults for each question are maked with a '*'. Press <enter> to accept the default.)")
        log.info("")
        
        tui.header("PLUG-IN INSTALLATION FOR HPLIP %s" % version)

        if plugin_installed:
            log.info("The driver plugin for HPLIP %s appears to already be installed." % version)

            cont, ans = tui.enter_yes_no("Do you wish to download and re-install the plug-in?")

            if not cont or not ans:
                clean_exit(0)


        if plugin_path is None:
            table = tui.Formatter(header=('Option', 'Description'), min_widths=(10, 50))
            table.add(('d', 'Download plug-in from HP (recommended)'))
            table.add(('p', 'Specify a path to the plug-in (advanced)'))
            table.add(('q', 'Quit hp-plugin (skip installation)'))

            table.output()

            cont, ans = tui.enter_choice("\nEnter option (d=download*, p=specify path, q=quit) ? ",
                ['d', 'p','q'], 'd')

            if not cont or ans == 'q': # q
                clean_exit(0)


            if ans == 'd': # d - download
                plugin_path = ""

            else : # p - specify plugin path
                while True:
                    plugin_path = input(log.bold("Enter the path to the 'hplip-%s-plugin.run' file (q=quit) : " %
                        version)).strip()

                    if plugin_path.strip().lower() == 'q':
                        clean_exit(1)

                    if  plugin_path.startswith('http://'):
                        log.error("Plug-in filename =%s must be local file." % plugin_path)
                        continue

                    else:
                        plugin_path = os.path.normpath(os.path.abspath(os.path.expanduser(plugin_path)))

                        if not os.path.exists(plugin_path):
                            log.error("Plug-in path '%s' not found." % plugin_path)
                            continue

                        if os.path.isdir(plugin_path):
                            plugin_path = os.path.join(plugin_path, plugin_filename)

                            if not os.path.exists(plugin_path):
                                log.error("Plug-in path '%s' not found." % plugin_path)
                                continue

                        if os.path.basename(plugin_path) != plugin_filename:
                            log.error("Plug-in filename must be '%s'." % plugin_filename)
                            continue

                        size, checksum, timestamp = os.stat(plugin_path)[6], '', 0.0
                        plugin_path = 'file://' + plugin_path

                    break


        if plugin_path.startswith('file://'):
            tui.header("COPY PLUGIN")
        else:
            tui.header("DOWNLOAD PLUGIN")
            log.info("Checking for network connection...")
            ok = utils.check_network_connection()

            if not ok:
                log.error("Network connection not detected.")
                clean_exit(1)

        log.info("Downloading plug-in from: %s" % plugin_path)
        pm = tui.ProgressMeter("Downloading plug-in:")

        status, plugin_path, error_str = pluginObj.download(plugin_path, plugin_download_callback)
        print()


        if status != ERROR_SUCCESS:

            log.error(error_str)

            if status in (ERROR_UNABLE_TO_RECV_KEYS, ERROR_DIGITAL_SIGN_NOT_FOUND):
                cont, ans = tui.enter_yes_no("Do you still want to install the plug-in?", 'n')

                if not cont or not ans:
                    pluginObj.deleteInstallationFiles(plugin_path)
                    clean_exit(0)
            else:
                pluginObj.deleteInstallationFiles(plugin_path)
                clean_exit(1)


        tui.header("INSTALLING PLUG-IN")

        pluginObj.run_plugin(plugin_path, mode)
        pluginObj.deleteInstallationFiles(plugin_path)

        cups_devices = device.getSupportedCUPSDevices(['hp']) #, 'hpfax'])
        #print cups_devices

        title = False

        for dev in cups_devices:
            mq = device.queryModelByURI(dev)

            if mq.get('fw-download', 0):

                if not title:
                    tui.header("DOWNLOADING FIRMWARE")
                    title = True

                # Download firmware if needed
                log.info(log.bold("\nDownloading firmware to device %s..." % dev))
                try:
                    d = device.Device(dev)
                except Error:
                    log.error("Error opening device. Exiting.")
                    clean_exit(1)

                if d.downloadFirmware():
                    log.info("Firmware download successful.\n")

                d.close()


    except KeyboardInterrupt:
        log.error("User exit")

log.info("")
log.info("Done.")
clean_exit(0)

