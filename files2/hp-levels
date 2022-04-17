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
__title__ = 'Supply Levels Utility'
__mod__ = 'hp-levels'
__doc__ = "Display bar graphs of current supply levels for supported HPLIP printers."

# Std Lib
import sys
import getopt
import time
import operator
import os

# Local
from base.g import *
from base import device, status, utils, tui, module
from prnt import cups

DEFAULT_BAR_GRAPH_SIZE = 8*(tui.ttysize()[1])/10


def logBarGraph(agent_level, agent_type, size=DEFAULT_BAR_GRAPH_SIZE, use_colors=True, bar_char='/'):
    #print agent_level, agent_type, size, use_colors, bar_char

    adj = 100.0/size
    if adj==0.0: adj=100.0
    bar = int(agent_level/adj)
    size = int(size)
    if bar > (size-2): bar = size-2

    if use_colors:
        if agent_type in (AGENT_TYPE_CMY, AGENT_TYPE_KCM, AGENT_TYPE_CYAN, AGENT_TYPE_CYAN_LOW):
            log.info(log.codes['teal'])
        elif agent_type in (AGENT_TYPE_MAGENTA, AGENT_TYPE_MAGENTA_LOW):
            log.info(log.codes['fuscia'])
        elif agent_type in (AGENT_TYPE_YELLOW, AGENT_TYPE_YELLOW_LOW):
            log.info(log.codes['yellow'])
        elif agent_type == AGENT_TYPE_BLUE:
            log.info(log.codes['blue'])
        elif agent_type in (AGENT_TYPE_BLACK, AGENT_TYPE_BLACK_B8800):
            log.info(log.codes['bold'])
        elif agent_type in (AGENT_TYPE_LG, AGENT_TYPE_G, AGENT_TYPE_PG):
            pass

    color = ''
    if use_colors:
        if agent_type in (AGENT_TYPE_CMY, AGENT_TYPE_KCM):
            color = log.codes['fuscia']

    log.info(("-"*(size))+color)

    color = ''
    if use_colors:
        if agent_type in (AGENT_TYPE_CMY, AGENT_TYPE_KCM):
            color = log.codes['yellow']

    log.info("%s%s%s%s (approx. %d%%)%s" % ("|", bar_char*bar,
             " "*((size)-bar-2), "|", agent_level, color))


    color = ''
    if use_colors:
        color = log.codes['reset']

    log.info(("-"*int(size))+color)
    #log.info(("-"*(size))+color)


log.set_module('hp-levels')

try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, None,
                        (INTERACTIVE_MODE,))

    mod.setUsage(module.USAGE_FLAG_DEVICE_ARGS,
        extra_options=[
        ("Bar graph size:", "-s<size> or --size=<size> (current default=%d)" % DEFAULT_BAR_GRAPH_SIZE, "option", False),
        ("Use colored bar graphs:", "-c or --color (default is colorized)", "option", False),
        ("Bar graph character:", "-a<char> or --char=<char> (default is '/')", "option", False)])


    opts, device_uri, printer_name, mode, ui_toolkit, lang = \
        mod.parseStdOpts('s:ca:', ['size=', 'color', 'char='])

    device_uri = mod.getDeviceUri(device_uri, printer_name)
    if not device_uri:
        sys.exit(1)
    log.info("Using device : %s\n" % device_uri)
    size = DEFAULT_BAR_GRAPH_SIZE
    color = True
    bar_char = '/'

    for o, a in opts:
        if o in ('-s', '--size'):
            try:
                size = int(a.strip())
            except (TypeError, ValueError):
                log.warn("Invalid size specified. Using the default of %d" % DEFAULT_BAR_GRAPH_SIZE)
                size = DEFAULT_BAR_GRAPH_SIZE

            if size < 1 or size > DEFAULT_BAR_GRAPH_SIZE:
                log.warn("Invalid size specified. Using the default of %d" % DEFAULT_BAR_GRAPH_SIZE)
                size = DEFAULT_BAR_GRAPH_SIZE

        elif o in ('-c', '--color'):
            color = True

        elif o in ('-a', '--char'):
            try:
                bar_char = a[0]
            except KeyError:
                bar_char = '/'


    try:
        d = device.Device(device_uri, printer_name)
    except Error:
        log.error("Error opening device. Exiting.")
        sys.exit(1)

    try:
        try:
            d.open()
            d.queryDevice()
        except Error as e:
            log.error("Error opening device (%s). Exiting." % e.msg)
            sys.exit(1)

        if d.mq['status-type'] != STATUS_TYPE_NONE:
            log.info("")

            sorted_supplies = []
            a = 1
            while True:
                try:
                    agent_type = int(d.dq['agent%d-type' % a])
                    agent_kind = int(d.dq['agent%d-kind' % a])
                    agent_sku = d.dq['agent%d-sku' % a]
                    log.debug("%d: agent_type %d agent_kind %d agent_sku '%s'" % (a, agent_type, agent_kind, agent_sku))
                except KeyError:
                    break
                else:
                    sorted_supplies.append((a, agent_kind, agent_type, agent_sku))
                a += 1
            sorted_supplies.sort(key=utils.cmp_to_key(utils.levelsCmp))

            for x in sorted_supplies:
                a, agent_kind, agent_type, agent_sku = x
                agent_health = d.dq['agent%d-health' % a]
                agent_level = d.dq['agent%d-level' % a]
                agent_desc = d.dq['agent%d-desc' % a]
                agent_health_desc = d.dq['agent%d-health-desc' % a]

                if agent_health in (AGENT_HEALTH_OK, AGENT_HEALTH_UNKNOWN) and \
                    agent_kind in (AGENT_KIND_SUPPLY,
                                    AGENT_KIND_HEAD_AND_SUPPLY,
                                    AGENT_KIND_TONER_CARTRIDGE,
                                    AGENT_KIND_MAINT_KIT,
                                    AGENT_KIND_ADF_KIT,
                                    AGENT_KIND_INT_BATTERY,
                                    AGENT_KIND_DRUM_KIT,):

                    log.info(log.bold(agent_desc))
                    log.info("Part No.: %s" % agent_sku)
                    log.info("Health: %s" % agent_health_desc)
                    logBarGraph(agent_level, agent_type, size, color, bar_char)
                    log.info("")

                else:
                    log.info(log.bold(agent_desc))
                    log.info("Part No.: %s" % agent_sku)
                    log.info("Health: %s" % agent_health_desc)
                    log.info("")


        else:
            log.error("Status not supported for selected device.")
            sys.exit(1)
    finally:
        d.close()

except KeyboardInterrupt:
    log.error("User exit")

log.info("")
log.info("Done.")



