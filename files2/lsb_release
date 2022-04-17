#!/usr/bin/python3 -Es

# lsb_release command for Debian
# (C) 2005-10 Chris Lawrence <lawrencc@debian.org>

#    This package is free software; you can redistribute it and/or modify
#    it under the terms of the GNU General Public License as published by
#    the Free Software Foundation; version 2 dated June, 1991.

#    This package is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU General Public License for more details.

#    You should have received a copy of the GNU General Public License
#    along with this package; if not, write to the Free Software
#    Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA
#    02110-1301 USA

from optparse import OptionParser
import sys
import os
import re

import lsb_release

def main():
    parser = OptionParser()
    parser.add_option('-v', '--version', dest='version', action='store_true',
                      default=False,
                      help="show LSB modules this system supports")
    parser.add_option('-i', '--id', dest='id', action='store_true',
                      default=False,
                      help="show distributor ID")
    parser.add_option('-d', '--description', dest='description',
                      default=False, action='store_true',
                      help="show description of this distribution")
    parser.add_option('-r', '--release', dest='release',
                      default=False, action='store_true',
                      help="show release number of this distribution")
    parser.add_option('-c', '--codename', dest='codename',
                      default=False, action='store_true',
                      help="show code name of this distribution")
    parser.add_option('-a', '--all', dest='all',
                      default=False, action='store_true',
                      help="show all of the above information")
    parser.add_option('-s', '--short', dest='short',
                      action='store_true', default=False,
                      help="show requested information in short format")
    
    (options, args) = parser.parse_args()
    if args:
        parser.error("No arguments are permitted")

    short = (options.short)
    none = not (options.all or options.version or options.id or
                options.description or options.codename or options.release)

    distinfo = lsb_release.get_distro_information()

    if none or options.all or options.version:
        verinfo = lsb_release.check_modules_installed()
        if not verinfo:
            print("No LSB modules are available.", file=sys.stderr)
        elif short:
            print(':'.join(verinfo))
        else:
            print('LSB Version:\t' + ':'.join(verinfo))

    if options.id or options.all:
        if short:
            print(distinfo.get('ID', 'n/a'))
        else:
            print('Distributor ID:\t%s' % distinfo.get('ID', 'n/a'))

    if options.description or options.all:
        if short:
            print(distinfo.get('DESCRIPTION', 'n/a'))
        else:
            print('Description:\t%s' % distinfo.get('DESCRIPTION', 'n/a'))

    if options.release or options.all:
        if short:
            print(distinfo.get('RELEASE', 'n/a'))
        else:
            print('Release:\t%s' % distinfo.get('RELEASE', 'n/a'))

    if options.codename or options.all:
        if short:
            print(distinfo.get('CODENAME', 'n/a'))
        else:
            print('Codename:\t%s' % distinfo.get('CODENAME', 'n/a'))

if __name__ == '__main__':
    main()
