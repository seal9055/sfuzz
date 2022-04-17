#!/usr/bin/python3
# -*- coding: utf-8 -*-
#
# (c) Copyright 2012-2020 HP Development Company, L.P.
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

__version__ = '1.0'
__title__ = 'Self Diagnse Utility and Healing Utility'
__mod__ = 'hp-doctor'
__doc__ = """Tool checks for the deprecated, plug-in, dependencies, queues, permission issues and provides self diagnose steps"""


# global import
import getopt
import os
import sys
import getpass

#local import
from base.g import *
from base.strings import *
try:
    from base import utils, tui, module,queues, os_utils, services, smart_install
except ImportError as e:
    if 'cupsext' in e.args[0] :
        check_extension_module_env('cupsext')
    else:
        log.exception("")
        sys.exit(1)
        
from installer.core_install import *
from check import DependenciesCheck

USAGE = [(__doc__, "", "name", True),
         ("Usage: %s [OPTIONS]" % __mod__, "", "summary", True),
         utils.USAGE_SPACE,
         utils.USAGE_MODE,
         ("Run in interactive mode:", "-i or --interactive (Default)", "option", False),
#         ("Run in graphical UI mode:", "-u or --gui (future use)", "option", False),
         utils.USAGE_SPACE,
         utils.USAGE_OPTIONS,
         utils.USAGE_HELP,
         utils.USAGE_LOGGING1, utils.USAGE_LOGGING2, utils.USAGE_LOGGING3,
#         ("Non-interactive mode:","-n(Without asking permissions)(future use)","option",False),
#         ("Perform the task for the given device id:","-d<device id>(future use)","option",False),
#         ("Take options from the file instead of command line:","-f<file> (future use)","option",False)

        ]

##########################global variables ##########################3
MODE = INTERACTIVE_MODE
DEVICE_URI = None
PERFORM_IN_NON_INTERACTIVE_MODE=False
LOG_LEVEL=None
VALID_AUTHENTICATION = False
IS_RESTART_REQ = False
DONOT_CLOSE_TERMINAL=False
SUMMARY_ONLY = False

#################################### functions #########################
def usage(typ='text'):
    if typ == 'text':
        utils.log_title(__title__, __version__)

    utils.format_text(USAGE, typ, __title__, __mod__, __version__)
    clean_exit(2)


def append_options(cmd):
    if MODE == INTERACTIVE_MODE:
        cmd += " -i "
    elif MODE == GUI_MODE:
        cmd += " -u "

    if PERFORM_IN_NON_INTERACTIVE_MODE:
        cmd += " -n "

    if LOG_LEVEL:
        cmd += " -l%s"%LOG_LEVEL

    # Adding quiet mode option..
    cmd += " -s "
    return cmd


def authenticate(core):
    global VALID_AUTHENTICATION
    if not services.running_as_root() and VALID_AUTHENTICATION == False:
        ###TBD
        # if MODE == GUI_MODE:
        #    GUI passwrd query..
        # else:
        if core.passwordObj.getAuthType() == "sudo":
            tui.title("ENTER SUDO PASSWORD")
        else:
            tui.title("ENTER ROOT/SUPERUSER PASSWORD")

        VALID_AUTHENTICATION = core.check_password()
    else:
        VALID_AUTHENTICATION = True

    if not VALID_AUTHENTICATION:
        log.error("3 incorrect attempts. (or) Insufficient permissions(i.e. try with sudo user).\nExiting.")
        clean_exit(3)

    return VALID_AUTHENTICATION


def install_plugin(core):
    plugin_sts = core.get_plugin_status()
    if plugin_sts == PLUGIN_VERSION_MISMATCH:
        ok,user_input =tui.enter_choice("Found Plugin version mismatch. Press 'y' to re-install the plugin(y=yes*, n=no):",['y', 'n'], 'y')
    elif plugin_sts == PLUGIN_FILES_CORRUPTED:
        ok,user_input =tui.enter_choice("Plugins corrupted. Press 'y' to re-install the plugin(y=yes*, n=no):",['y', 'n'], 'y')
    elif plugin_sts == PLUGIN_NOT_INSTALLED:
        ok,user_input =tui.enter_choice("Plugin's are missing. Press 'y' to install the plugin(y=yes*, n=no):",['y', 'n'], 'y')
    elif plugin_sts == PLUGIN_INSTALLED:
        log.info("Plugin's already installed")
        return True
    else:
        log.info("No plug-in printers are configured.")
        return True

    if ok and user_input == 'y':
#        authenticate(core)
        cmd='hp-plugin'
        cmd = append_options(cmd)
        sts = os_utils.execute(cmd)
        if sts == 0:
            return True
        else:
            log.info(log.bold("Failed to install Plugin. Please run 'hp-plugin' command to install plugin manually"))
    return False


def deprecated_check(core):
    if core.validate_distro_version():
        log.debug("This distro is supported.")
        log.info("No Deprecated items are found")
    else:
        log.error("This distro (i.e %s  %s) is either deprecated or not yet supported."%(core.distro_name, core.distro_version))
        ok,user_input =tui.enter_choice(log.red("The diagnosis is limited on unsupported platforms. Do you want to continue?(y=yes*, n=no):"),['y', 'n'], 'y')
        if not ok or user_input !='y':
            clean_exit(2)


def display_missing_dependencies(required_dependencies=[],optional_dependencies=[], missing_cmd=[]):
    if len(required_dependencies):
        log.info(log.bold("Missing Required Dependencies"))
        log.info(log.bold('-'*len("Missing Required Dependencies")))
        for packages_to_install in required_dependencies:
           if 'cups' in packages_to_install:
               log.error("'%s' package is missing or '%s' service is not running."%(packages_to_install,'cups'))
           else:
               log.error("'%s' package is missing/incompatible "%packages_to_install)

    if len(optional_dependencies):
        log.info(log.bold("Missing Optional Dependencies"))
        log.info(log.bold('-'*len("Missing Optional Dependencies")))
        for packages_to_install in optional_dependencies:
            log.error("'%s' package is missing/incompatible "%packages_to_install)

    if len(missing_cmd):
        log.info(log.bold("Missing Commands"))
        log.info(log.bold('-'*len("Missing Commands")))
        for cmd in missing_cmd:
            log.error("'%s' is missing"%cmd)


def clean_exit(exit_code=0):
    mod.unlockInstance()

    if DONOT_CLOSE_TERMINAL:
        log.info("\n\nPlease close this terminal manually. ")
        try:
            while 1:
                pass
        except KeyboardInterrupt:
            pass

    sys.exit(exit_code)


#################################### Main #########################
log.set_module(__mod__)
try:
    mod = module.Module(__mod__, __title__, __version__, __doc__, USAGE,
                    (INTERACTIVE_MODE, GUI_MODE),
                    (UI_TOOLKIT_QT3, UI_TOOLKIT_QT4, UI_TOOLKIT_QT5), True)

    opts, device_uri, printer_name, mode, ui_toolkit, loc = \
               mod.parseStdOpts('hl:gnid:f:w', ['summary-only','help', 'help-rest', 'help-man', 'help-desc', 'interactive', 'gui', 'lang=','logging=', 'debug'],
                     handle_device_printer=False)

except getopt.GetoptError as e:
    log.error(e.msg)
    usage()

if os.getenv("HPLIP_DEBUG"):
    log.set_level('debug')
    LOG_LEVEL = 'debug'

for o, a in opts:
    if o == '-n':
        MODE = NON_INTERACTIVE_MODE
        PERFORM_IN_NON_INTERACTIVE_MODE = True
        log.warn("NON_INTERACTIVE mode is not yet supported.")
        #TBD
        usage()
    elif o == '-d':
        DEVICE_URI=a
    elif o in ('-u', '--gui'):
        log.warn("GUI is not yet supported.")
        #TBD
        usage()
    elif o == '-f':
        log.warn("Option from file is not yet supported")
        #TBD
        usage()
    elif o in ('-l', '--logging'):
        LOG_LEVEL = a.lower().strip()
        if not log.set_level(LOG_LEVEL):
            usage()
    elif o == '-w':
        DONOT_CLOSE_TERMINAL = True

    elif o == '--summary-only':
        SUMMARY_ONLY = True


try:
    if os.geteuid() == 0:
        log.error("%s %s"  %(__mod__, queryString(ERROR_RUNNING_AS_ROOT)))
        sys.exit(1)

    mod.lockInstance('')
    mod.quiet= False
    mod.showTitle()
    log_file = os.path.normpath('%s/hp-doctor.log'%prop.user_dir)

    if os.path.exists(log_file):
        try:
            os.remove(log_file)
        except OSError:
            pass

    log.set_logfile(log_file)
    log.set_where(log.LOG_TO_CONSOLE_AND_FILE)

    log.debug("Upgrade log saved in: %s" % log.bold(log_file))
    log.debug("")

    if PERFORM_IN_NON_INTERACTIVE_MODE and os.geteuid() != 0:
        log.error("Non Interactive mode should be run in root mode.")
        clean_exit(1)

    ui_toolkit = sys_conf.get('configure','ui-toolkit')

    dep =  DependenciesCheck(MODE_CHECK,INTERACTIVE_MODE,ui_toolkit)
    dep.core.init()
    log.info(log.bold("\n\nChecking for Deprecated items...."))

    deprecated_check(dep.core)

    log.info(log.bold("\n\nChecking for HPLIP updates...."))
    upgrade_cmd = utils.which('hp-upgrade',True)
    if upgrade_cmd:
        #checking for latest version of HPLIP.
        upgrade_cmd = append_options(upgrade_cmd)
        sts = os_utils.execute(upgrade_cmd)
        if sts != 0:
            log.error("Failed to upgrade latest HPLIP. Is hp-upgrade already running (i.e. foreground or background)?")
    else:
        log.error("Failed to locate hp-upgrade utility")

    ### Dependency check
    log.info(log.bold("\n\nChecking for Dependencies...."))
    if SUMMARY_ONLY:
        num_errors, num_warns = dep.validate(DEPENDENCY_RUN_AND_COMPILE_TIME, True)
    else:
        num_errors, num_warns = dep.validate(DEPENDENCY_RUN_AND_COMPILE_TIME, False)

    if num_errors or num_warns:

        if dep.get_required_deps() or dep.get_optional_deps() or dep.get_cmd_to_run():
            display_missing_dependencies(dep.get_required_deps(),dep.get_optional_deps(), dep.get_cmd_to_run())
            authenticate(dep.core)
            dep.core.install_missing_dependencies(INTERACTIVE_MODE,dep.get_required_deps(),dep.get_optional_deps(), dep.get_cmd_to_run())

        log.info(log.bold("\n\nChecking Permissions...."))
#        if not core.get_missing_user_grps() and not core.get_disable_selinux_status():
        # if not core.get_disable_selinux_status():
        #     log.info("Permissions are correct.")

#        if core.get_missing_user_grps():
#            log.info(log.bold("Missing User Groups"))
#            log.info(log.bold('-'*len("Missing User Groups")))
#            log.info("%s"%core.get_missing_user_grps())
#            authenticate(core)
#            if core.add_groups_to_user(core.get_missing_user_grps(), core.get_user_grp_cmd()):
#                IS_RESTART_REQ = True

        # if core.get_disable_selinux_status():
        #     log.info(log.bold("SELinux Status"))
        #     log.info(log.bold('-'*len("SELinux Status")))
        #     log.info("SELinux is enabled. Needs to be disabled")
        #     authenticate(core)
        #     if core.disable_SELinux():
        #         IS_RESTART_REQ = True

    log.info(log.bold("\n\nChecking for Configured Queues...."))
    queues.main_function(dep.core.passwordObj, MODE,ui_toolkit, False, DEVICE_URI)

    log.info(log.bold("\n\nChecking for HP Properitery Plugin's...."))
    ### Check for Plugin Printers
    install_plugin(dep)

    smart_ins_dev_list = smart_install.get_smartinstall_enabled_devices()
    if smart_ins_dev_list:
        log.info(log.bold("\n\nChecking for 'CD-ROM'/'Smart Install' Detected Devices...."))
        url, tool_name = smart_install.get_SmartInstall_tool_info()
        for printer in smart_ins_dev_list:
            log.error("Smart Install is Enabled in '%s' Printer. This needs to be disabled."%printer)
        log.info(log.bold("\nRefer link '%s' to disable Smart Install manually.\n"%(url)))

    comm_err_dev = dep.get_communication_error_devs()
    if comm_err_dev:
        log.info(log.bold("\n\nChecking for Printer Status...."))
        for printer in comm_err_dev:
            log.error("'%s' Printer is either Powered-OFF or Failed to communicate."%printer)
            log.info(log.bold("Turn On Printer and re-run %s"%__mod__))

    if IS_RESTART_REQ:
        log.info(log.bold("\nPlease reboot the system before performing any function."))

    log.info(log.bold("\nDiagnose completed...\n"))
    log.info("")
    log.info("")
    log.info("More information on Troubleshooting,How-To's and Support is available on http://hplipopensource.com/hplip-web/index.html")
        
    clean_exit(0)


except KeyboardInterrupt:
    log.error("User exit")
    clean_exit(1)

