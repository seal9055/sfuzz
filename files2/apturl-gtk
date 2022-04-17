#!/usr/bin/python3
#
# Copyright (c) 2007-2008 Canonical
#
# AUTHOR:
# Michael Vogt <mvo@ubuntu.com>
# With contributions by Siegfried-A. Gevatter <rainct@ubuntu.com>
#
# This file is part of AptUrl
#
# AptUrl is free software; you can redistribute it and/or
# modify it under the terms of the GNU General Public License as published
# by the Free Software Foundation; either version 2 of the License, or (at
# your option) any later version.
#
# AptUrl is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
# General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with AptUrl; if not, write to the Free Software
# Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA

# ignore apt's "API not stable yet" warning
import warnings
warnings.filterwarnings("ignore", category=FutureWarning, append=1)

import sys
import gettext
from gettext import gettext as _

from AptUrl.AptUrl import AptUrlController, RESULT_CANCELT
from AptUrl.gtk.GtkUI import GtkUI


if __name__ == "__main__":
    localesApp="apturl"
    localesDir="/usr/share/locale"
    gettext.bindtextdomain(localesApp, localesDir)
    gettext.textdomain(localesApp)

    ui = GtkUI()
    apturl = AptUrlController(ui)

    try:
        sys.exit(apturl.main())
    except KeyboardInterrupt:
        print(_("User requested interrupt."))
        sys.exit(RESULT_CANCELT)
