#! /usr/bin/python3
# -*- coding: utf-8 -*-
# (c) 2012 Canonical Ltd.
#
# Authors: Alberto Milone <alberto.milone@canonical.com>
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
# You should have received a copy of the GNU General Public License along
# with this program; if not, write to the Free Software Foundation, Inc.,
# 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

import optparse
import os
import sys
import logging

import Quirks.quirkapplier


# Here's where we look for quirks
quirks_path = '/usr/share/ubuntu-drivers-common/quirks'

def main(options):
    if options.verbose:
        loglevel = logging.DEBUG
    else:
        loglevel = logging.INFO

    logging.basicConfig(format='%(levelname)s:%(message)s', level=loglevel)

    if options.package_enable and options.package_disable:
        sys.exit(1)
    elif options.package_enable and not options.package_disable:
        logging.info('Enable %s' % options.package_enable)
        quirks = Quirks.quirkapplier.QuirkChecker(options.package_enable, path=quirks_path)
        quirks.enable_quirks()
    elif not options.package_enable and options.package_disable:
        logging.info('Disable %s' % options.package_disable)
        quirks = Quirks.quirkapplier.QuirkChecker(options.package_disable, path=quirks_path)
        quirks.disable_quirks()
    else:
        print('no args')

if __name__ == '__main__':
    parser = optparse.OptionParser()
    parser.add_option("-e", "--enable-quirks", dest="package_enable",
                      help="enable quirks for package", metavar="FILE")
    parser.add_option("-d", "--disable-quirks", dest="package_disable",
                      help="disable quirks for package", metavar="FILE")
    parser.add_option("-v", "--verbose",
                      action="store_true", dest="verbose", default=False,
                      help="show debug messages")

    (options, args) = parser.parse_args()
    
    operation_status = main(options)

    #sys.exit(operation_status)

