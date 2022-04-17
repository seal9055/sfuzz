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
# Authors: Don Welch, Smith Kennedy
#

__version__ = '5.0'
__title__ = 'Device URI Creation Utility'
__mod__ = 'hp-makeuri'
__doc__ = "Creates device URIs for local and network connected printers for use with CUPS."

# Std Lib
import sys
import re
import getopt
import os

# Local
from base.g import *
from base.codes import *
from base import device, utils, module


USAGE = [ (__doc__, "", "name", True),
          ("Usage: %s [OPTIONS] [SERIAL NO.|USB ID|IP|DEVNODE]" % __mod__, "", "summary", True),
          ("[SERIAL NO.|USB ID|IP|DEVNODE]", "", "heading", False),
          ("USB IDs (usb only):", """"xxx:yyy" where xxx is the USB bus ID and yyy is the USB device ID. The ':' and all leading zeroes must be present.""", 'option', False),
          ("", """(Use the 'lsusb' command to obtain this information. See Note 1.)""", "option", False),
          ("IPs (network only):", 'IPv4 address "a.b.c.d" or "hostname"', "option", False),
          ("DEVNODE (parallel only):", '"/dev/parportX", X=0,1,2,...', "option", False),
          ("SERIAL NO. (usb and parallel only):", '"serial no."', "option", True),
          utils.USAGE_OPTIONS,
          ("To specify the port on a multi-port JetDirect:", "-p<port> or --port=<port> (Valid values are 1\*, 2, and 3. \*default)", "option", False),
          ("Show the CUPS URI only (quiet mode):", "-c or --cups", "option", False),
          ("Show the SANE URI only (quiet mode):", "-s or --sane", "option", False),
          ("Show the HP Fax URI only (quiet mode):", "-f or --fax", "option", False),
          utils.USAGE_LOGGING1, utils.USAGE_LOGGING2, utils.USAGE_LOGGING3,
          utils.USAGE_HELP,
          utils.USAGE_EXAMPLES,
          ("USB:", "$ hp-makeuri 001:002", "example", False),
          ("Network:", "$ hp-makeuri 66.35.250.209", "example", False),
          ("Parallel:", "$ hp-makeuri /dev/parport0", "example", False),
          ("USB or parallel (using serial number):", "$ hp-makeuri US123456789", "example", False),
          utils.USAGE_SPACE,
          utils.USAGE_NOTES,
          ("1. Example using 'lsusb' to obtain USB bus ID and USB device ID (example only, the values you obtain will differ) :", "", 'note', False),
          ("   $ lsusb", "", 'note', False),
          ("   Bus 003 Device 011: ID 03f0:c202 Hewlett-Packard", "", 'note', False),
          ("   $ hp-makeuri 003:011", "", 'note', False),
          ("   (Note: You may have to run 'lsusb' from /sbin or another location. Use '$ locate lsusb' to determine this.)", "", 'note', True),
          utils.USAGE_SPACE,
          utils.USAGE_SEEALSO,
          ("hp-setup", "", "seealso", False),
        ]


mod = module.Module(__mod__, __title__, __version__, __doc__, USAGE, 
                    (INTERACTIVE_MODE,), None, True, True)

opts, device_uri, printer_name, mode, ui_toolkit, lang = \
    mod.parseStdOpts('p:csf', ['port', 'cups', 'sane', 'fax'],
                     handle_device_printer=False)

try:
    cups_quiet_mode = False
    sane_quiet_mode = False
    fax_quiet_mode = False
    jd_port = 1

    for o, a in opts:
        if o in ('-c', '--cups'):
            cups_quiet_mode = True

        elif o in ('-s', '--sane'):
            sane_quiet_mode = True

        elif o in ('-f', '--fax'):
            fax_quiet_mode = True

        elif o in ('-p', '--port'):
            try:
                jd_port = int(a)
            except ValueError:
                mod.usage(error_msg=["Invalid port number. Must be between 1 and 3 inclusive."])

        elif o == '-g':
            log.set_level('debug')


    quiet_mode = cups_quiet_mode or sane_quiet_mode or fax_quiet_mode
    mod.quiet = quiet_mode
    
    #if quiet_mode:
    #    log.set_level('warn')

    #utils.log_title(__title__, __version__) 
    mod.showTitle()

    if len(mod.args) != 1:
        mod.usage(error_msg=["You must specify one SERIAL NO., IP, USB ID or DEVNODE parameter."])

    param = mod.args[0]

    if 'localhost' in param.lower():
        mod.usage(error_msg=['Invalid hostname'])

    cups_uri, sane_uri, fax_uri = device.makeURI(param, jd_port)

    if not cups_uri:
        log.error("Device not found")
        sys.exit(1)

    if cups_quiet_mode:
        print(cups_uri)

    elif not quiet_mode:    
        print("CUPS URI: %s" % cups_uri)

    if sane_uri:
        if sane_quiet_mode:
            print(sane_uri)
        
        elif not quiet_mode:
            print("SANE URI: %s" % sane_uri)
    
    elif not sane_uri and sane_quiet_mode:
        log.error("Device does not support scan.")

    if fax_uri:
        if fax_quiet_mode:
            print(fax_uri)
        
        elif not quiet_mode:
            print("HP Fax URI: %s" % fax_uri)
            
    elif not fax_uri and fax_quiet_mode:
        log.error("Device does not support fax.")

except KeyboardInterrupt:
    log.error("User exit")

if not quiet_mode:
    log.info("")
    log.info("Done.")

