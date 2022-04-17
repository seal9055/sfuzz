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
# Author: Don Welch, Sarbeswar Meher
#

__version__ = '2.4'
__title__ = 'Firmware Download Utility'
__mod__ = 'hp-firmware'
__doc__ = "Download firmware to a device that requires downloaded firmware to function. (Note: Most printers do not require the use of this utility)."

# Std Lib
import sys
import getopt
import gzip
import operator
import time
import os


# Local
from base.g import *
from base import device, status, utils, tui, module
from prnt import cups

try:
    from importlib import import_module
except ImportError as e:
    log.debug(e)
    from base.utils import dyn_import_mod as import_module


try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (INTERACTIVE_MODE, GUI_MODE, NON_INTERACTIVE_MODE),
                        (UI_TOOLKIT_QT5, UI_TOOLKIT_QT4, UI_TOOLKIT_QT3), True, True)

    mod.setUsage(module.USAGE_FLAG_DEVICE_ARGS,
        extra_options=[
        ("Use USB IDs to specify printer:", "-s bbb:ddd, where bbb is the USB bus ID and ddd is the USB device ID. The ':' and all leading zeroes must be present.", "option", False),
        ("Seconds to delay before download:", "-y<secs> or --delay=<secs> (float value, e.g. 0.5)", "option", False)],
         see_also_list=['hp-plugin', 'hp-toolbox'])

    device_uri = None
    printer_name = None
    usb_bus_node = None
    usb_bus_id = None
    usb_device_id = None
    silent = False
    delay = 0.0

    opts, device_uri, printer_name, mode, ui_toolkit, lang = \
        mod.parseStdOpts('y:s:', ['delay='])

    for o, a in opts:
        if o == '-s':
            silent = True
            try:
                usb_bus_id, usb_device_id = a.split(":", 1)
                log.debug("USB bus ID: %s" % usb_bus_id)
                log.debug("USB device ID: %s" % usb_device_id)
            except ValueError:
                log.error("Invalid USB IDs: %s" % a)
                sys.exit(1)

            if len(usb_bus_id) != 3 or len(usb_device_id) != 3:
                log.error("Invalid USB IDs '%s'. Must be the format: bbb.ddd" % a)
                sys.exit(1)

            usb_bus_node = a
            mode = NON_INTERACTIVE_MODE

        elif o in ('-y', '--delay'):
            try:
                delay = float(a)
            except ValueError:
                log.error("Invalid delay value. Must be numeric (float) value. Setting delay to 0.0")
                delay = 0.0

            mode = NON_INTERACTIVE_MODE


    if mode == GUI_MODE and (ui_toolkit == 'qt4' or ui_toolkit == 'qt5'):
        if not utils.canEnterGUIMode4():
            log.error("%s -u/--gui requires Qt4/Qt5 GUI support. Entering interactive mode." % __mod__)
            mode = INTERACTIVE_MODE

    elif mode == GUI_MODE and ui_toolkit == 'qt3':
       if not utils.canEnterGUIMode():
            log.error("%s -u/--gui requires Qt3 GUI support. Entering interactive mode." % __mod__)
            mode = INTERACTIVE_MODE

    if mode in (GUI_MODE, INTERACTIVE_MODE):
        mod.quiet = False

    if mode == GUI_MODE:
        if ui_toolkit == 'qt4'or ui_toolkit == 'qt5':
           # try:
           #  from PyQt4.QtGui import QApplication
           #  from ui4.firmwaredialog import FirmwareDialog
           # except ImportError:
           #  log.error("Unable to load Qt4 support. Is it installed?")
           #  sys.exit(1)
            QApplication, ui_package = utils.import_dialog(ui_toolkit)
            ui = import_module(ui_package + ".firmwaredialog")

        if ui_toolkit == 'qt3':
           try:
            from qt import *
            from ui.firmwaredialog import FirmwareDialog
           except ImportError:
            log.error("Unable to load Qt3 support. Is it installed?")
            sys.exit(1)


        mod.showTitle()

        device_uri = mod.getDeviceUri(device_uri, printer_name,
            filter={'fw-download': (operator.gt, 0)})

        if device_uri:
            app = QApplication(sys.argv)
            dialog = ui.FirmwareDialog(None, device_uri)
            dialog.show()
            try:
                log.debug("Starting GUI loop...")
                if ui_toolkit == 'qt4' or ui_toolkit == 'qt5':
                   app.exec_()
                elif ui_toolkit == 'qt3':
                   dialog.exec_loop()
            except KeyboardInterrupt:
                sys.exit(0)
        
        sys.exit(0)

    mod.showTitle()

    if usb_bus_node is not None:
        log.debug("USB bus node: %s" % usb_bus_node)
        device_uri, sane_uri, fax_uri = device.makeURI(usb_bus_node, 1)

        if not device_uri:
            log.error("Invalid USB Device ID or USB bus ID. No device found.")
            sys.exit(1)

    else:
        device_uri = mod.getDeviceUri(device_uri, printer_name,
            filter={'fw-download': (operator.gt, 0)})

        if not device_uri:
            sys.exit(1)

    try:
        d = device.Device(device_uri, printer_name)
    except Error:
        log.error("Error opening device. Exiting.")
        sys.exit(1)

    try:
        if delay:
             time.sleep(delay)

        try:
            d.open()
            d.queryModel()
        except Error as e:
            log.error("Error opening device (%s). Exiting." % e.msg)
            sys.exit(1)

        fw_download = d.mq.get('fw-download', 0)

        if fw_download:
            if d.downloadFirmware(usb_bus_id, usb_device_id):
                if not silent:
                    log.info("Done.")
                sys.exit(0)

            else:
                log.error("Firmware download failed.")
                sys.exit(1)

        else:
            log.error("Device %s does not support or require firmware download." % device_uri)
            sys.exit(1)

    finally:
        d.close()

except KeyboardInterrupt:
    log.error("User exit")
