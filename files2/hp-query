#!/usr/bin/python3
# -*- coding: utf-8 -*-
#
# (c) Copyright 2015 HP Development Company, L.P.
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
from __future__ import print_function
__version__ = '0.2'
__title__ = 'Model Query Utility'
__mod__ = 'hp-query'
__doc__ = "Query a printer model for static model information. Designed to be called from other processes."

# Std Lib
import sys

# Local
from base.g import *
from base import device, models, module


try:

    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (NON_INTERACTIVE_MODE,),  quiet=True)

    mod.setUsage(0,
        extra_options=[
        ("Specify model by device URI:", "-d<device_uri> or --device=<device_uri>", "option", False),
        ("Specify normalized model name:", "-m<model_name> or --model=<model_name> (normalized models.dat format)", "option", False),
        ("Specify raw model name:", "-r<model_name> or --raw=<model_name> (raw model name from MDL: field of device ID)", "option", False),
        ("Specify key to query:", "-k<key> or --key=<key> (or, use -a/--all to return all keys)", "option", False),
        ("Query all keys:", "-a or --all (default separator is a LF)", "option", False),
        ("Specify the separator when multiple keys are queried:", "-s<sep> --sep=<sep> (character or 'tab', 'newline', 'cr', 'lf', 'crlf')(only valid when used with -a/--all)", "option", False),
        ("Suppress trailing linefeed:", "-x", "option", False),],
        see_also_list=['hp-info'])

    opts, device_uri, printer_name, mode, ui_toolkit, lang = \
        mod.parseStdOpts('m:k:as:d:r:x', ['model=', 'key=', 'sep=', 'all', 'device=', 'raw='],
        handle_device_printer=False)

    norm_model = None
    raw_model = None
    device_uri = None
    key = None
    all_keys = False
    sep = 'lf'
    suppress_trailing_linefeed = False

    for o, a in opts:
        if o in ('-m', '--model'):
            norm_model = a

        elif o in ('-d', '--model'):
            device_uri = a

        elif o in ('-k', '--key'):
            key = a
            all_keys = False

        elif o in ('-a', '--all'):
            all_keys = True
            key = None

        elif o in ('-r', '--raw'):
            raw_model = a

        elif o in ('-s', '--sep'):
            sep = a

        elif o == '-x':
            suppress_trailing_linefeed = True

    if (device_uri and norm_model) or \
       (device_uri and raw_model) or \
       (norm_model and raw_model):
        log.stderr("error: You may only specify one of -d, -m, or -r.")
        sys.exit(1)

    if not device_uri and not norm_model and not raw_model:
        log.stderr("error: You must specify one of -d, -m, or -r.")
        sys.exit(1)

    if device_uri:
        try:
            back_end, is_hp, bus, norm_model, serial, dev_file, host, zc, port = \
                device.parseDeviceURI(device_uri)
        except Error:
            log.stderr("error: Invalid device URI: %s" % device_uri)
            sys.exit(1)

    elif raw_model:
        norm_model = models.normalizeModelName(raw_model).lower()

    if not norm_model:
        log.stderr("error: Invalid model name.")
        sys.exit(1)

    s = sep.lower()
    if s in ('lf', 'newline'):
        sep = '\n'
    elif s == 'cr':
        sep = '\r'
    elif s == 'crlf':
        sep = '\r\n'
    elif s == 'tab':
        sep = '\t'
    elif s == '=':
        log.stderr("error: Separator must not be '='.")
        sys.exit(1)

    data = device.queryModelByModel(norm_model)

    if not data:
        log.stderr("error: Model name '%s' not found." % norm_model)
        sys.exit(1)

    output = ''
    if all_keys:
        kk = list(data.keys())
        kk.sort()
        for k in kk:
            if not output:
                output = '%s=%s' % (k, data[k])
            else:
                output = sep.join([output, '%s=%s' % (k, data[k])])

    elif key:
        try:
            data[key]
        except KeyError:
            log.stderr("error: Key '%s' not found." % key)
            sys.exit(1)
        else:
            output = '%s=%s' % (key, data[key])

    else:
        log.stderr("error: Must specify key with -k/--key or specify -a/--all.")
        sys.exit(1)

    if suppress_trailing_linefeed:
        print(output, end=' ')
    else:
        print(output)


except KeyboardInterrupt:
    pass



