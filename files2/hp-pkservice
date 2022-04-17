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

__version__ = '1.0'
__mod__ = 'hp-pkservice'
__title__ = 'Policy Kit Service'
__doc__ = "Policy Kit Service"

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
from base import utils, module

USAGE = [ (__doc__, "", "name", True),
          ("Usage: %s [MODE] [OPTIONS]" % __mod__, "", "summary", True),
          utils.USAGE_MODE,
          utils.USAGE_INTERACTIVE_MODE,
          utils.USAGE_LANGUAGE,
          utils.USAGE_OPTIONS,
          utils.USAGE_LOGGING1, utils.USAGE_LOGGING2, utils.USAGE_LOGGING3,
          utils.USAGE_HELP,
          utils.USAGE_SPACE,
        ]

mod = module.Module(__mod__, __title__, __version__, __doc__, USAGE,
                    (INTERACTIVE_MODE, ), run_as_root_ok=True)

mod.setUsage(module.USAGE_FLAG_NONE,
    extra_options=[utils.USAGE_SPACE,
    ("[OPTIONS] (General)", "", "header", False),
    ("PolicyKit version:", "-v<version> or --version=<version>", "option", False)])

opts, device_uri, printer_name, mode, ui_toolkit, loc = \
    mod.parseStdOpts('v:', ["version="])

user_pkit_version = None

for o, a in opts:
    if o in ('-v', '--version'):
        try:
            user_pkit_version = int(a)
        except:
            log.error("-v or --version require an integer argument")
            sys.exit(1)
        if user_pkit_version < 0 or user_pkit_version > 1:
            log.error("invalid PolicyKit version...use 0 or 1")
            sys.exit(1)

PKIT = utils.to_bool(sys_conf.get('configure', 'policy-kit'))
if PKIT:
    try:
        from base.pkit import *
        pkit_version = policykit_version()
        if not user_pkit_version is None:
            pkit_version = user_pkit_version
        try:
            from dbus.mainloop.glib import DBusGMainLoop
        except ImportError:
            log.error("PolicyKit requires dbus")
            sys.exit(1)
    except:
        log.error("Unable to load pkit...is HPLIP installed?")
        sys.exit(1)
else:
    log.error("PolicyKit support not installed")
    sys.exit(1)

DBusGMainLoop(set_as_default=True)

if not os.geteuid() == 0:
    log.error("You must be root to run this utility.")
    sys.exit(1)

log.debug("using PolicyKit version %d" % pkit_version)

try:
    BackendService().run(pkit_version)
except dbus.DBusException as ex:
    log.error("Unable to start service (%s)" % ex)
