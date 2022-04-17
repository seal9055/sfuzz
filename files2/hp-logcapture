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
# Author: Amarnath Chitumalla
#
from __future__ import print_function
__version__ = '1.0'
__title__ = 'HPLIP logs capture Utility'
__mod__ = 'hp-logcapture'
__doc__ = """Captures the HPLIP log files."""

import os
import sys
import getopt
import glob
import datetime

from base.g import *
from base import utils,tui,module, os_utils
from base.sixext import to_string_utf8
from subprocess import Popen, PIPE
from installer.core_install import *

CUPS_FILE='/etc/cups/cupsd.conf'
CUPS_BACKUP_FILE='/etc/cups/cupsd.conf_orginal'
LOG_FOLDER_PATH='./'
LOG_FOLDER_NAME='hplip_troubleshoot_logs'
LOG_FILES=LOG_FOLDER_PATH + LOG_FOLDER_NAME
TMP_DIR = "/var/spool/cups/tmp"
USER_NAME =""
USERS={}
################ is_journal() function ##############
#Capture logs from system journal for Fedora 21 onwards

def is_journal():
    core =  CoreInstall(MODE_INSTALLER, INTERACTIVE_MODE)
    core.get_distro()
    distro_name = core.distro_name
    distro_ver = core.distro_version
    if distro_name == "fedora" and distro_ver >=" 21" :
        journal = True
    else:
        journal = False
    return journal

############ enable_log() function ############
#This function changes CUPS conf log level to debug and restarts CUPS service.

def enable_log():
    result = False
    cmd='cp -f %s %s'%(CUPS_FILE,CUPS_BACKUP_FILE)
    log.debug("Backup CUPS conf file. cmd =%s"%cmd)
    sts,out=utils.run(cmd)
    if sts != 0:
        log.error("Failed to take back cups file=%s"%CUPS_FILE)

    #check if cups is log level enabled or disable
    cmd="grep 'LogLevel warn' %s"%CUPS_FILE
    log.debug ("cmd= %s"%cmd)
    sts,out=utils.run(cmd)
    if sts == 0:
        cmd = "sed -i 's/LogLevel.*warn/LogLevel debug\rhpLogLevel 15/' %s "%CUPS_FILE
        log.debug("Changing 'Log level' to debug. cmd=%s"%cmd)
        sts= os.system(cmd)
        if sts != 0:
           log.error("Failed to update Loglevel to Debug in cups=%s"%CUPS_FILE)

        cmd=None
        if utils.which('service'):
           cmd = os.path.join(utils.which('service'), 'service')+" cups restart"
        elif utils.which('systemctl'):
           cmd = os.path.join(utils.which('systemctl'), 'systemctl')+" restart %s.service"%service_name
        elif os.path.exists('/etc/init.d/cups'):
           cmd = "/etc/init.d/cups restart"
        else:
           log.error("service command not found.. Please restart cups manually..")

        if cmd:
           log.debug("CUPS restart cmd = %s"%cmd)
           sts,out = utils.run(cmd)
           if sts == 0:
               result = True

    return result

############ restore_loglevels() function ############
#This function restores CUPS conf file to previous value and restarts CUPS service.

def restore_loglevels():
    result = False
    cmd='cp -f %s %s'%(CUPS_BACKUP_FILE,CUPS_FILE)
    log.debug("Restoring CUPS conf file. cmd=%s"%cmd)
    sts, out = utils.run(cmd)
    if sts == 0:
       cmd='rm -f %s'%CUPS_BACKUP_FILE
       log.debug("Removing Temporary file.. cmd=%s"%cmd)
       sts,out = utils.run(cmd)
       if sts != 0:
            log.warn("Failed to remove the Temporary backup file=%s"%CUPS_BACKUP_FILE)
    else:
       log.error("Failed to restore cups config file = %s"%CUPS_FILE)
    log.debug("Restarting CUPS service")

    cmd=None
    if utils.which('service'):
       cmd = os.path.join(utils.which('service'), 'service')+" cups restart"
    elif utils.which('systemctl'):
       cmd = os.path.join(utils.which('systemctl'), 'systemctl')+" restart %s.service"%service_name
    elif os.path.exists('/etc/init.d/cups'):
       cmd = "/etc/init.d/cups restart"
    else:
       log.error("service command not found.. Please restart cups manually..")

    if cmd:
        log.debug("CUPS restart cmd = %s"%cmd)
        sts,out = utils.run(cmd)
        if sts == 0:
           result = True

    return result

def usage(typ='text'):
    if typ == 'text':
        utils.log_title(__title__, __version__)

    utils.format_text(USAGE, typ, __title__, __mod__, __version__)
    sys.exit(0)


def backup_clearLog(strLog):
    if os.path.exists(strLog):
        iArch =1
        while os.path.exists("%s.%d"%(strLog, iArch)) or os.path.exists("%s.%d.gz"%(strLog, iArch)):
            iArch +=1
        sts,out = utils.run('cp %s %s.%d'%(strLog, strLog, iArch))
        if sts != 0:
            log.error("Failed to archive %s log file"%strLog)
        else:
            cmd = 'cat /dev/null > %s' % strLog
            sts = os_utils.execute(cmd)
            if sts != 0:
                log.warn("Failed to clear the %s log file"%strLog)
            if utils.which('gzip'):
                sts,out = utils.run ('gzip %s.%d'%(strLog, iArch))
                if sts != 0:
                    log.info("Existing %s log file copied to %s.%d"%(strLog, strLog, iArch))
                else:
                    log.info("Existing %s log file copied to %s.%d.gz"%(strLog, strLog, iArch))
            else:
                log.info("Existing %s log file copied to %s.%d"%(strLog, strLog, iArch))



USAGE = [(__doc__, "", "name", True),
         ("Usage: [su -c /sudo] %s [USER INFO] [OPTIONS]" % __mod__, "", "summary", True),
         ("e.g. su -c '%s'"%__mod__,"","summary",True),
         ("[USER INFO]", "", "heading", False),
         ("User name for which logs to be collected:", "--user=<username> ", "option", False),
         utils.USAGE_OPTIONS,
         utils.USAGE_HELP,
         utils.USAGE_LOGGING1, utils.USAGE_LOGGING2, utils.USAGE_LOGGING3,
        ]


######## Main #######
try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, USAGE,
                    (INTERACTIVE_MODE,),run_as_root_ok=True, quiet=True)

    opts, device_uri, printer_name, mode, ui_toolkit, loc = \
               mod.parseStdOpts('hl:g:r', ['help', 'help-rest', 'help-man', 'help-desc', 'logging=', 'debug','user='],handle_device_printer=False)
except getopt.GetoptError as e:
    log.error(e.msg)
    usage()

if os.getenv("HPLIP_DEBUG"):
    log.set_level('debug')

for o, a in opts:
    if o in ('-h', '--help'):
        usage()

    elif o == '--help-rest':
        usage('rest')

    elif o == '--help-man':
        usage('man')

    elif o == '--help-desc':
        print(__doc__, end=' ')
        clean_exit(0,False)

    elif o in ('-l', '--logging'):
        log_level = a.lower().strip()
        if not log.set_level(log_level):
            usage()

    elif o in ('-g', '--debug'):
        log.set_level('debug')

    elif o == '--user':
        USER_NAME = a



if os.getuid() != 0:
    log.error("logCapture needs root permissions since cups service restart requires....")
    sys.exit()

if not USER_NAME:
    pout = Popen(["who"], stdout=PIPE)
    output = to_string_utf8(pout.communicate()[0])
    if output:
        USER_NAME = output.split(' ')[0]

    if not USER_NAME:
        log.error("Failed to get the user name. Try again by passing '--user' option")
        sys.exit(1)

if not os.path.exists(TMP_DIR):
    TMP_DIR = "/tmp"

cmd = "mkdir -p %s"%LOG_FILES
log.debug("Creating temporary logs folder =%s"%cmd)
sts, out = utils.run(cmd)
if sts != 0:
   log.error("Failed to create directory =%s. Exiting"%LOG_FILES)
   sys.exit(1)

sts,out = utils.run('chmod 755  %s'%LOG_FILES)
if sts != 0:
    log.error("Failed to change permissions for %s."%(LOG_FILES))


USERS[USER_NAME]="/home/"+USER_NAME+"/.hplip"

USERS['root']="/root/.hplip"
for u in USERS:
    sts, out = utils.run('mkdir -p %s/%s'%(LOG_FILES,u))
    if sts != 0:
       log.error("Failed to create directory =%s. Exiting"%LOG_FILES)
       sys.exit(1)

    sts,out = utils.run('chmod 755  %s/%s'%(LOG_FILES,u))
    if sts != 0:
        log.error("Failed to change permissions for %s/%s."%(LOG_FILES,u))


enable_log()

#### Clearing previous logs.. ###########
if not is_journal():
    ok,user_input = tui.enter_choice("Archiving system logs (i.e. syslog, message, error_log). Press (y=yes*, n=no, q=quit):",['y', 'n','q'], 'y')
    if not ok or user_input == "q":
        restore_loglevels()
        log.warn("User exit")
        sys.exit(1)

    if ok and user_input == "y":
        backup_clearLog('/var/log/syslog')
        backup_clearLog('/var/log/messages')
        backup_clearLog('/var/log/cups/error_log')



######## Waiting for user to completed job #######
while 1:
    log_time = datetime.datetime.strftime(datetime.datetime.now(),'%Y-%m-%d %H:%M:%S')
    log.info(log.bold("\nPlease perform the tasks (Print, scan, fax) for which you need to collect the logs."))
    ok,user_input =tui.enter_choice("Are you done with tasks?. Press (y=yes*, q=quit):",['y','q'], 'y')
    if ok and user_input == "y":
        break;
    elif not ok or user_input == "q":
        restore_loglevels()
        log.warn("User exit")
        sys.exit(1)

######## Copying logs to Temporary log folder #######
sts,out = utils.run('hp-check')
if sts != 0:
    log.error("Failed to run hp-check command")

log.debug("Copying logs to Temporary folder =%s"%LOG_FILES)
if not is_journal():
    if os.path.exists('/var/log/syslog'):
        sts,out = utils.run ('cp -f /var/log/syslog %s/syslog.log'%LOG_FILES)
        if sts != 0:
           log.error("Failed to capture %s log file."%("/var/log/syslog"))

    if os.path.exists('/var/log/messages'):
        sts,out = utils.run('cp -f /var/log/messages %s/messages.log'%LOG_FILES)
        if sts != 0:
           log.error("Failed to capture %s log file."%("/var/log/messages"))

    if os.path.exists('/var/log/cups/error_log'):
        sts,out = utils.run('cp -f /var/log/cups/error_log %s/cups_error_log.log'%LOG_FILES)
        if sts != 0:
           log.error("Failed to capture %s log file."%("/var/log/cups/error_log"))
else:
    log.debug("Collecting cups logs from system journal")
    cmd = "journalctl -u cups.service -e --since '%s' " %log_time
    sts = os.system(cmd + "> %s/cups_error.log"%LOG_FILES)
    if sts != 0:
        log.error("Failed to capture logs from journal")


    log.debug("Collecting messages from system journal")
    cmd = "journalctl --since '%s' " %log_time
    sts = os.system(cmd + "> %s/messages.log"%LOG_FILES)
    if sts != 0:
        log.error("Failed to capture messages from journal")

for u in USERS:
    sts = os.system('cp -f %s/*.log  %s/%s 2>/devnull '%(USERS[u],LOG_FILES,u))

sts,out = utils.run('mv -f ./hp-check.log %s'%LOG_FILES)
if sts != 0:
    log.error("Failed to capture %s log files."%("./hp-check.log"))
cmd = 'chmod 666  %s/*.log' % LOG_FILES
sts = os_utils.execute(cmd)
if sts != 0:
    log.error("Failed to change permissions for %s."%(LOG_FILES))

######## Compressing log files #######
cmd = 'tar -zcf %s.tar.gz %s'%(LOG_FOLDER_NAME,LOG_FILES)
log.debug("Compressing logs. cmd =%s"%cmd)

sts_compress,out = utils.run(cmd)
if sts_compress != 0:
    log.error("Failed to compress %s folder."%(LOG_FILES))
else:
    log.debug("Changing Permissions of ./%s.tar.gz "%LOG_FOLDER_NAME)
    sts,out = utils.run('chmod 666 -R ./%s.tar.gz'%(LOG_FOLDER_NAME))
    if sts != 0:
        log.error("Failed to change permissions for %s.tar.gz."%(LOG_FILES))
    log.debug("Removing Temporary log files..")
    sts,out = utils.run('rm -rf %s'%LOG_FILES)
    if sts != 0:
        log.error("Failed to remove temporary files. Remove manually."%(LOG_FILES))

restore_loglevels()

log.info("")
log.info("")
if sts_compress == 0:
    log.info(log.bold("Logs are saved as %s/%s.tar.gz"%( os.getcwd(),LOG_FOLDER_NAME)))
    log.info(log.bold("Please create a bug @https://bugs.launchpad.net/hplip/+filebug and upload this log file."))
else:
    log.info(log.bold("Logs are saved as %s/%s"%(os.getcwd(),LOG_FOLDER_NAME)))
log.info("")
