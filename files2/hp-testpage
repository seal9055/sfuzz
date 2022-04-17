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

__version__ = '6.0'
__title__ = 'Testpage Print Utility'
__mod__ = 'hp-testpage'
__doc__ = "Print a tespage to a printer. Prints a summary of device information and shows the printer's margins."

# Std Lib
import sys
import os
import getopt
import re
import time


# Local
from base.g import *
from base import device, utils, tui, module
from prnt import cups


try:
    from importlib import import_module
except ImportError as e:
    log.debug(e)
    from base.utils import dyn_import_mod as import_module



try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (INTERACTIVE_MODE, GUI_MODE),
                        (UI_TOOLKIT_QT4, UI_TOOLKIT_QT5))

    mod.setUsage(module.USAGE_FLAG_DEVICE_ARGS)

    opts, device_uri, printer_name, mode, ui_toolkit, loc = \
        mod.parseStdOpts()

    wait_for_printout = False
    sts, printer_name, device_uri = mod.getPrinterName(printer_name, device_uri)

    if not sts:
        log.error("No installed printers found (or) Invalid printer device selected")
        sys.exit(1)

    if mode == GUI_MODE:
        if not utils.canEnterGUIMode4():
            log.error("%s -u/--gui requires Qt4 GUI support. Entering interactive mode." % __mod__)
            mode = INTERACTIVE_MODE

    if mode == GUI_MODE:
        # try:
        #     from PyQt4.QtGui import QApplication
        #     from ui4.printtestpagedialog import PrintTestPageDialog
        # except ImportError:
        #     log.error("Unable to load Qt4 support. Is it installed?")
        #     sys.exit(1)
        QApplication, ui_package = utils.import_dialog(ui_toolkit)
        ui = import_module(ui_package + ".printtestpagedialog")

        log.set_module("%s(UI)" % __mod__)

        if 1:
            app = QApplication(sys.argv)
            dialog = ui.PrintTestPageDialog(None, printer_name)
            dialog.show()
            try:
                log.debug("Starting GUI loop...")
                app.exec_()
            except KeyboardInterrupt:
                sys.exit(0)

        sys.exit(0)

    if mode == INTERACTIVE_MODE:
    #else: # INTERACTIVE_MODE
        try:
            d = device.Device(device_uri, printer_name)
        except Error as e:
            log.error("Device error (%s)." % e.msg)
            sys.exit(1)

        try:
            try:
                d.open()
            except Error:
                log.error("Unable to print to printer. Please check device and try again.")
                sys.exit(1)

            # TODO: Fix the wait for printout stuff... can't get device ID
            # while hp: backend has device open in printing mode...
            wait_for_printout = False

            if d.isIdleAndNoError():
                d.close()
                log.info( "Printing test page to printer %s..." % printer_name)
                try:
                    d.printTestPage(printer_name)
                except Error as e:
                    if e.opt == ERROR_NO_CUPS_QUEUE_FOUND_FOR_DEVICE:
                        log.error("No CUPS queue found for device. Please install the printer in CUPS and try again.")
                    else:
                        log.error("An error occured (code=%d)." % e.opt)
                else:
                    if wait_for_printout:
                        log.info("Test page has been sent to printer. Waiting for printout to complete...")

                        time.sleep(5)
                        i = 0

                        while True:
                            time.sleep(5)

                            try:
                                d.queryDevice(quick=True)
                            except Error as e:
                                log.error("An error has occured.")

                            if d.error_state == ERROR_STATE_CLEAR:
                                break

                            elif d.error_state == ERROR_STATE_ERROR:
                                cleanup_spinner()
                                log.error("An error has occured (code=%d). Please check the printer and try again." % d.status_code)
                                break

                            elif d.error_state == ERROR_STATE_WARNING:
                                cleanup_spinner()
                                log.warning("There is a problem with the printer (code=%d). Please check the printer." % d.status_code)

                            else: # ERROR_STATE_BUSY
                                update_spinner()

                            i += 1

                            if i > 24:  # 2min
                                break

                        cleanup_spinner()

                    else:
                        log.info("Test page has been sent to printer.")

            else:
                log.error("Device is busy or in an error state. Please check device and try again.")
                sys.exit(1)


        finally:
            d.close()

            log.info("")
            log.notice("If an error occured, or the test page failed to print, refer to the HPLIP website")
            log.notice("at: http://hplip.sourceforge.net for troubleshooting and support.")
            log.info("")

except KeyboardInterrupt:
    log.error("User exit")

log.info("")
log.info("Done.")
