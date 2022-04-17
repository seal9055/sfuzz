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

__version__ = '3.0'
__title__ = 'Printer Cartridge Color Calibration Utility'
__mod__ = 'hp-colorcal'
__doc__ = "Perform color calibration on HPLIP supported inkjet printers. (Note: Not all printers require the use of this utility)."

# Std Lib
import sys
import re
import getopt
import operator
import os

# Local
from base.g import *
from base import device, status, utils, maint, tui, module
from base.sixext.moves import input
from prnt import cups


def enterAlignmentNumber(letter, hortvert, colors, minimum, maximum):
    return tui.enter_range("Enter the best aligned value for line %s (%d-%d or q=quit): " %
        (letter, minimum, maximum), minimum, maximum)

def enterPaperEdge(maximum):
    return tui.enter_range("Enter numbered arrow that is best aligned with the paper edge (1-%d or q=quit): " %
        maximum, 1, maximum)

def colorAdj(line, maximum):
    return tui.enter_range("Enter the numbered box on line %s that is best color matched to the background color (1-%d or q=quit): " %
        (line, maximum), 1, maximum)

def colorCal():
    return tui.enter_range("""Enter the numbered image labeled "1" thru "7" that is best color matched to the image labeled "X""", 1, 7)

def colorCal2():
    return tui.enter_range("""Select the number between 1 and 81 of the numbered patch that best matches the background.""", 1, 81)

def invalidPen():
    log.error("Invalid cartridge(s) installed.\nPlease install valid cartridges and try again.")

def photoPenRequired():
    log.error("Photo cartridge not installed.\nPlease install the photo cartridge and try again.")

def photoPenRequired2():
    log.error("Photo cartridge or photo blue cartridge not installed.\nPlease install the photo (or photo blue) cartridge and try again.")


def colorCal4():
    log.info("""Instructions:\n1. Hold the calibration page at arm's length in front of your eyes.
2. Tilt the page away from you. Look at the two large squares, each containing colored patches. For each large square, find the colored path that most closely matches the background color. Each patch has an associated letter and number.
""")
    values = [0, 0, 0, 0]
    ok = True
    while True:
        x = input(log.bold("""Enter the letter ('A' thru 'N') and number (1 thru 14) for the GRAY plot (eg, "C5") or "q" to quit: """))

        if x.lower().strip() == 'q':
            ok = False
            break

        if x.lower().strip() == 'd': # use defaults
            values[0], values[1] = -1, -1
            break

        if len(x) < 2:
            log.error("You must enter at least two characters (a letter and a number)")
            continue

        if len(x) > 3:
            log.error('Enter only a single letter and a one or two digit number (eg, "C5").')
            continue

        letter = x[0].lower()

        if letter not in 'abcdefghijklmn':
            log.error("You must enter a letter 'A' thru 'N'")
            continue

        try:
            number = int(x[1:])
        except ValueError:
            log.error("You must enter a letter 'A' thru 'N' followed by a number 1 thru 14.")
            continue

        if number < 0 or number > 14:
            log.error("You must enter a letter 'A' thru 'N' followed by a number 1 thru 14.")
            continue

        values[0] = ord(str(letter).upper()) - ord('A')
        values[1] = number - 1
        break

    if ok:
        while True:
            x = input(log.bold("""Enter the letter ('P' thru 'V') and number (1 thru 7) for the COLOR plot (eg, "R3") or "q" to quit: """))

            if x.lower().strip() == 'q':
                ok = False
                break

            if x.lower().strip() == 'd': # use defaults
                values[2], values[3] = -1, -1
                break

            if len(x) < 2:
                log.error("You must enter at least two characters (a letter and a number)")
                continue

            if len(x) > 3:
                log.error('Enter only a single letter and a one or two digit number (eg, "R3").')
                continue

            letter = x[0].lower()

            if letter not in 'pqrstuv':
                log.error("You must enter a letter 'P' thru 'V'")
                continue

            try:
                number = int(x[1:])
            except ValueError:
                log.error("You must enter a letter 'P' thru 'V' followed by a number 1 thru 7.")
                continue

            if number < 0 or number > 7:
                log.error("You must enter a letter 'P' thru 'V' followed by a number 1 thru 7.")
                continue

            values[2] = ord(str(letter).upper()) - ord('P')
            values[3] = number - 1
            break

    return ok, values


try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (INTERACTIVE_MODE, GUI_MODE), (UI_TOOLKIT_QT4,), False, False, True)

    mod.setUsage(module.USAGE_FLAG_DEVICE_ARGS,
                 see_also_list=['hp-align', 'hp-clean', 'hp-linefeedcal',
                                'hp-pqdiag'])

    opts, device_uri, printer_name, mode, ui_toolkit, lang = \
        mod.parseStdOpts()

    device_uri = mod.getDeviceUri(device_uri, printer_name,
        filter={'color-cal-type': (operator.ne, COLOR_CAL_TYPE_NONE)})

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
            sys.exit(1)

        try:
            try:
                d.open()
            except Error:
                log.error("Unable to print to printer. Please check device and try again.")
                sys.exit(1)

            if d.isIdleAndNoError():
                color_cal_type = d.mq.get('color-cal-type', COLOR_CAL_TYPE_NONE)
                log.debug("Color calibration type=%d" % color_cal_type)

                if color_cal_type == COLOR_CAL_TYPE_UNSUPPORTED:
                    log.error("Color calibration through HPLIP not supported for this printer. Please use the printer's front panel to perform color calibration.")

                elif color_cal_type == COLOR_CAL_TYPE_DESKJET_450: #1
                    maint.colorCalType1(d, tui.load_paper_prompt, colorCal, photoPenRequired)

                elif color_cal_type == COLOR_CAL_TYPE_MALIBU_CRICK: #2
                    maint.colorCalType2(d, tui.load_paper_prompt, colorCal2, invalidPen)

                elif color_cal_type == COLOR_CAL_TYPE_STRINGRAY_LONGBOW_TORNADO: #2
                    maint.colorCalType3(d, tui.load_paper_prompt, colorAdj, photoPenRequired2)

                elif color_cal_type == COLOR_CAL_TYPE_CONNERY: # 4
                    maint.colorCalType4(d, tui.load_paper_prompt, colorCal4, None)

                elif color_cal_type == COLOR_CAL_TYPE_COUSTEAU: # 5
                    maint.colorCalType5(d, tui.load_paper_prompt)

                elif color_cal_type == COLOR_CAL_TYPE_CARRIER: # 6
                    maint.colorCalType6(d, tui.load_paper_prompt)

                elif color_cal_type == COLOR_CAL_TYPE_TYPHOON: # 7
                    maint.colorCalType7(d, tui.load_photo_paper_prompt)

                else:
                    log.error("Invalid color calibration type.")

            else:
                log.error("Device is busy or in an error state. Please check device and try again.")
                sys.exit(1)
        finally:
            d.close()

    else:
        try:
            from PyQt4.QtGui import QApplication
            from ui4.colorcaldialog import ColorCalDialog
        except ImportError:
            log.error("Unable to load Qt4 support. Is it installed?")
            sys.exit(1)

        #try:
        if 1:
            app = QApplication(sys.argv)
            dlg = ColorCalDialog(None, device_uri)
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
log.info('Done.')
