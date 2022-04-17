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

__version__ = '4.0'
__title__ = 'Printer Printhead Cleaning Utility'
__mod__ = 'hp-clean'
__doc__ = "Printhead cleaning utility for HPLIP supported inkjet printers."

#Std Lib
import sys
import re
import getopt
import time
import operator
import os

# Local
from base.g import *
from base import device, utils, maint, tui, module
from prnt import cups

try:
    from importlib import import_module
except ImportError as e:
    log.debug(e)
    from base.utils import dyn_import_mod as import_module


def CleanUIx(level):
    global d
    ok = tui.continue_prompt("Ready to perform level %d cleaning ." % level)

    if ok:
        timeout = 0
        time.sleep(5)

        try:
            while True:
                update_spinner()
                try:
                    d.open()
                except Error:
                    time.sleep(2)
                    timeout += 2
                    continue

                if d.isIdleAndNoError():
                    break

                time.sleep(1)
                timeout += 1

                if timeout > 45:
                    log.error("Timeout waiting for print to finish.")
                    sys.exit(0)


        finally:
            cleanup_spinner()
            d.close()

    return ok

def CleanUI1(msg=""):
    if not msg:
        log.note("Please wait for page to complete printing before continuing.\nLevel 1 cleaning complete. If the printout looks OK.")
        log.info("Note: Wait for previous print to finish") 
    else:
        log.note(msg)

    log.info("Press enter 'q' to quit or <enter> to do a level 2 cleaning.")
    return CleanUIx(2)


def CleanUI2(msg=""):
    if not msg:
        log.note("Please wait for page to complete printing before continuing.\nLevel 2 cleaning complete. If the printout looks OK.")
        log.info("Note: Wait for previous print to finish") 
    else:
        log.note(msg)

    log.info("Press enter 'q' to quit or <enter> to do a level 3 cleaning.")
    log.warn("Level 3 uses a lot of ink.")
    return CleanUIx(3)

def CleanUI3(msg =""):
    if msg:
        log.info(msg)
    else:
        log.info("\nLevel 3 cleaning complete. Check this page to see if the problem was fixed. If the test page was not printed OK, replace the printhead(s).")


try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (INTERACTIVE_MODE, GUI_MODE), (UI_TOOLKIT_QT4, UI_TOOLKIT_QT5))

    mod.setUsage(module.USAGE_FLAG_DEVICE_ARGS,
                 see_also_list=['hp-align', 'hp-clean', 'hp-linefeedcal',
                                'hp-pqdiag'])

    opts, device_uri, printer_name, mode, ui_toolkit, lang = \
        mod.parseStdOpts()

    device_uri = mod.getDeviceUri(device_uri, printer_name,
       filter={'clean-type': (operator.ne, CLEAN_TYPE_NONE)})

    if not device_uri:
        sys.exit(1)
    log.info("Using device : %s\n" % device_uri)
    if mode == GUI_MODE:
        if not utils.canEnterGUIMode4():
            log.error("%s -u/--gui requires Qt4 GUI support. Entering interactive mode." % __mod__)
            mode = INTERACTIVE_MODE

    if mode == INTERACTIVE_MODE:
        try:
            d = device.Device(device_uri, printer_name)
        except Error as e:
            log.error("Unable to open device: %s" % e.msg)
            sys.exit(0)

        try:
            try:
                d.open()
            except Error:
                log.error("Unable to print to printer. Please check device and try again.")
                sys.exit(1)

            if d.isIdleAndNoError():
                clean_type = d.mq.get('clean-type', CLEAN_TYPE_NONE)
                log.debug("Clean type=%d" % clean_type)
                d.close()

                try:
                    if clean_type == CLEAN_TYPE_UNSUPPORTED:
                        log.error("Cleaning through HPLIP not supported for this printer. Please use the printer's front panel to perform printhead cleaning.")

                    elif clean_type == CLEAN_TYPE_PCL:
                        maint.cleaning(d, clean_type, maint.cleanType1, maint.primeType1,
                                        maint.wipeAndSpitType1, tui.load_paper_prompt,
                                        CleanUI1, CleanUI2, CleanUI3,
                                        None)

                    elif clean_type == CLEAN_TYPE_LIDIL:
                        maint.cleaning(d, clean_type, maint.cleanType2, maint.primeType2,
                                        maint.wipeAndSpitType2, tui.load_paper_prompt,
                                        CleanUI1, CleanUI2, CleanUI3,
                                        None)

                    elif clean_type == CLEAN_TYPE_PCL_WITH_PRINTOUT:
                        maint.cleaning(d, clean_type, maint.cleanType1, maint.primeType1,
                                        maint.wipeAndSpitType1, tui.load_paper_prompt,
                                        CleanUI1, CleanUI2, CleanUI3,
                                        None)

                    elif clean_type == CLEAN_TYPE_LEDM:
                        maint.cleaning(d, clean_type, maint.cleanTypeLedm, maint.cleanTypeLedm1,
                                        maint.cleanTypeLedm2, tui.load_paper_prompt,
                                        CleanUI1, CleanUI2, CleanUI3,
                                        None, maint.isCleanTypeLedmWithPrint)

                    else:
                        log.error("Cleaning not needed or supported on this device.")

                except Error as e:
                    log.error("An error occured: %s" % e.msg)

            else:
                log.error("Device is busy or in an error state. Please check device and try again.")
                sys.exit(1)
        finally:
            d.close()

    else:

        QApplication, ui_package = utils.import_dialog(ui_toolkit)
        ui = import_module(ui_package + ".cleandialog")


        #try:
        if 1:
            app = QApplication(sys.argv)
            dlg = ui.CleanDialog(None, device_uri)
            dlg.show()
            try:
                log.debug("Starting GUI loop...")
                app.exec_()
            except KeyboardInterrupt:
                sys.exit(0)

        #finally:
        if 1:
            sys.exit(0)

except KeyboardInterrupt:
    log.error("User exit")

log.info("")
log.info("Done.")
