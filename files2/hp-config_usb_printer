#!/usr/bin/python3
# -*- coding: utf-8 -*-
#
# (c) Copyright 2011-2015 HP Development Company, L.P.
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
# Author: Amarnath Chitumalla
#

__version__ = '1.2'
__title__ = 'HP device config using USB'
__mod__ = 'hp-config_usb_printer'
__doc__ = "Udev invokes this tool. Tool detects the plugin, Smart Install (C/DVD-ROM) issues and notifies to logged-in user. Tool also downloads firmware to the device."

# Std Lib
import sys
import os

# Local
from base.g import *
from base import device, utils, module, services
from installer import pluginhandler


DBUS_SERVICE='com.hplip.StatusService'
DBUS_AVIALABLE=False

##### METHODS #####
# Send dbus event to hpssd on dbus system bus
def send_message(device_uri, printer_name, event_code, username, job_id, title, pipe_name=''):
    if DBUS_AVIALABLE == False:
        return

    log.debug("send_message() entered")
    args = [device_uri, printer_name, event_code, username, job_id, title, pipe_name]
    msg = lowlevel.SignalMessage('/', DBUS_SERVICE, 'Event')
    msg.append(signature='ssisiss', *args)

    SystemBus().send_message(msg)
    log.debug("send_message() returning")


# Usage function
def usage(typ='text'):
    utils.format_text(USAGE, typ, __title__, __mod__, __version__)
    sys.exit(0)


# Systray service. If hp-systray is not running, starts.
def start_systray():
    if DBUS_AVIALABLE == False:
        return False

    Systray_Is_Running=False
    status,output = utils.Is_Process_Running('hp-systray')
    if status is False:
        if os.getuid() == 0:
            log.error(" hp-systray must be running.\n Run \'hp-systray &\' in a terminal. ")
        else:
            log.info("Starting hp-systray service")
            services.run_systray()
            status,output = utils.Is_Process_Running('hp-systray')

    if status == True:
        Systray_Is_Running=True
        log.debug("hp-systray service is running\n")

    return Systray_Is_Running



USAGE = [ (__doc__, "", "name", True),
          ("Usage: %s [OPTIONS] [USB bus:device]" % __mod__, "", "summary", True),
          utils.USAGE_OPTIONS,
          utils.USAGE_LOGGING1, utils.USAGE_LOGGING2, utils.USAGE_LOGGING3,
          utils.USAGE_HELP,
          ("[USB bus:device]", "", "heading", False),
          ("USB bus:device :", """"xxx:yyy" where 'xxx' is the USB bus and 'yyy' is the USB device. (Note: The ':' and all leading zeros must be present.)""", 'option', False),
          ("", "Use the 'lsusb' command to obtain this information.", "option", False),
          utils.USAGE_EXAMPLES,
          ("USB, IDs specified:", "$%s 001:002"%(__mod__), "example", False),
          utils.USAGE_SPACE,
          utils.USAGE_NOTES,
          ("1. Using 'lsusb' to obtain USB IDs: (example)", "", 'note', False),
          ("   $ lsusb", "", 'note', False),
          ("         Bus 003 Device 011: ID 03f0:c202 Hewlett-Packard", "", 'note', False),
          ("   $ %s 003:011"%(__mod__), "", 'note', False),
          ("   (Note: You may have to run 'lsusb' from /sbin or another location. Use '$ locate lsusb' to determine this.)", "", 'note', True),
        ]


mod = module.Module(__mod__, __title__, __version__, __doc__, USAGE, (INTERACTIVE_MODE,), None, run_as_root_ok=True, quiet=True)
opts, device_uri, printer_name, mode, ui_toolkit, loc = mod.parseStdOpts('gh',['time-out=', 'timeout='],handle_device_printer=False)

LOG_FILE = "%s/hplip_config_usb_printer.log"%prop.user_dir
if os.path.exists(LOG_FILE):
    try:
        os.remove(LOG_FILE)
    except OSError:
        pass

log.set_logfile(LOG_FILE)
log.set_where(log.LOG_TO_CONSOLE_AND_FILE)

try:
    import dbus
    from dbus import SystemBus, lowlevel
except ImportError:
    log.warn("Failed to Import DBUS ")
    DBUS_AVIALABLE = False
else:
    DBUS_AVIALABLE = True

try:
    param = mod.args[0]
except IndexError:
    param = ''

log.debug("param=%s" % param)
if len(param) < 1:
    usage()
    sys.exit()

try:
    # ******************************* MAKEURI
    if param:
        device_uri, sane_uri, fax_uri = device.makeURI(param)
    if not device_uri:
        log.error("This is not a valid device")
        sys.exit(0)

    # ******************************* QUERY MODEL AND CHECKING SUPPORT
    log.debug("\nSetting up device: %s\n" % device_uri)
    mq = device.queryModelByURI(device_uri)
    if not mq or mq.get('support-type', SUPPORT_TYPE_NONE) == SUPPORT_TYPE_NONE:
        log.error("Unsupported printer model.")
        sys.exit(1)

    printer_name = ""
    username = prop.username
    job_id = 0

    # ******************************* Detecting smart install /CD-DVD ROM enable.
    if "SMART_INSTALL_ENABLED" in device_uri:
        if start_systray():
            send_message( device_uri, printer_name, EVENT_DIAGNOSE_PRINTQUEUE, username, job_id,'')
        else:
            log.error("SMART INSTALL (CD/DVD-ROM) is enabled in the system. Refer http://hplipopensource.com/hplip-web/index.html for more information.")

    # ******************************* TRIGGERING PLUGIN POP-UP FOR PLUGING SUPPORTED PRINTER'S
    plugin = mq.get('plugin', PLUGIN_NONE)
    if plugin != PLUGIN_NONE:
       pluginObj = pluginhandler.PluginHandle()
       plugin_sts = pluginObj.getStatus()
       if plugin_sts == pluginhandler.PLUGIN_INSTALLED:
          log.info("Device Plugin is already installed")
       elif plugin_sts == pluginhandler.PLUGIN_NOT_INSTALLED :
          log.info("HP Device Plug-in is not found")
       else:
          log.info("HP Device Plug-in version mismatch or some files are corrupted")

       if plugin_sts != pluginhandler.PLUGIN_INSTALLED:
           if start_systray():
               send_message( device_uri,  printer_name, EVENT_AUTO_CONFIGURE, username, job_id, "AutoConfig")
           else:
               log.error("HP Device plugin's are not installed. Please install plugin's using hp-plugin command.")

       # ******************************* RUNNING FIRMWARE DOWNLOAD TO DEVICE FOR SUPPORTED PRINTER'S
       fw_download_req = mq.get('fw-download', False)
       if fw_download_req:
           fw_cmd = "hp-firmware -y3 -s %s"%param
           log.info(fw_cmd)
           fw_sts, fw_out = utils.run(fw_cmd)
           if fw_sts == 0:
               log.debug("Firmware downloaded to %s "%device_uri)
           else:
               log.warn("Failed to download firmware to %s device"%device_uri)     

except KeyboardInterrupt:
    log.error("User exit")

log.debug("Done.")

