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

__version__ = '2.0'
__mod__ = 'hp-timedate'
__title__ = 'Time/Date Utility'
__doc__ = "Set the time and date on an HP Officejet all-in-one device using the PC time and date."

# Std Lib
import sys
import re
import getopt
import struct
import operator
import os

# Local
from base.g import *
from base.codes import *
from base import device, status, utils, pml, tui, module
from prnt import cups

try:
    from fax import faxdevice
except ImportError:
    log.error("Unable to load fax services for HPLIP (required for hp-timedate). Exiting.")
    sys.exit(1)


PML_ERROR_CODES = {
    pml.ERROR_OK_END_OF_SUPPORTED_OBJECTS: "OK: End of supported objects",
    pml.ERROR_OK_NEAREST_LEGAL_VALUE_SUBSITUTED: "OK: Nearest legal value substituted",
    pml.ERROR_UNKNOWN_REQUEST: "Unknown request",
    pml.ERROR_BUFFER_OVERFLOW: "Buffer overflow",
    pml.ERROR_COMMAND_EXECUTION: "Command execution",
    pml.ERROR_UNKNOWN_OID: "Unknown OID",
    pml.ERROR_OBJ_DOES_NOT_SUPPORT_SPECIFIED_ACTION: "Object does not support action",
    pml.ERROR_INVALID_OR_UNSUPPORTED_VALUE: "Invalid or unsupported value",
    pml.ERROR_PAST_END_OF_SUPPORTED_OBJS: "Past end of supported objects",
    pml.ERROR_ACTION_CANNOT_BE_PERFORMED_NOW: "Action cannot be performed now",
    pml.ERROR_SYNTAX: "Syntax",
}

try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (INTERACTIVE_MODE,))

    mod.setUsage(module.USAGE_FLAG_DEVICE_ARGS)

    opts, device_uri, printer_name, mode, ui_toolkit, lang = \
        mod.parseStdOpts()

    device_uri = mod.getDeviceUri(device_uri, printer_name,
        filter={'fax-type': (operator.gt, 0)},
        back_end_filter=['hpfax'])

    if not device_uri:
        sys.exit(1)
    log.info("Using device : %s\n" % device_uri)
    try:
        d = faxdevice.FaxDevice(device_uri, printer_name, disable_dbus=True)
    except Error as e:
        if e.opt == ERROR_DEVICE_DOES_NOT_SUPPORT_OPERATION:
            log.error("Device does not support setting time/date.")
            sys.exit(1)
        else:
            log.error(e.msg)
            sys.exit(1)

    try:
        try:
            d.open()
        except Error:
            log.error("Unable to open device. Exiting. ")
            sys.exit(1)

        try:
            log.info("Setting time and date on %s" % device_uri)
            d.setDateAndTime()
        except Error:
            log.error("An error occured!")
    finally:
        d.close()

except KeyboardInterrupt:
    log.error("User exit")

log.info("")
log.info('Done.')
