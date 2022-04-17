#! /usr/bin/python2
# -*- coding: UTF-8 -*- vim: et ts=4 sw=4
#
# Copyright © 2010-2012 Piotr Ożarowski <piotr@debian.org>
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in
# all copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
# THE SOFTWARE.

import logging
import optparse
import sys
from os import environ, remove
from os.path import exists
sys.path.insert(1, '/usr/share/python/')

from debpython import files as dpf
from debpython.namespace import add_namespace_files

# initialize script
logging.basicConfig(format='%(levelname).1s: %(module)s:%(lineno)d: '
                           '%(message)s')
log = logging.getLogger(__name__)

"""TODO: move it to manpage
Examples:
    pyclean -p python-mako # all .py[co] files from the package
    pyclean /usr/lib/python2.6/dist-packages # python2.6
"""


def destroyer():  # ;-)
    """Removes every .py[co] file associated to received .py file."""

    def find_files_to_remove(pyfile):
        for filename in ("%sc" % pyfile, "%so" % pyfile):
            if exists(filename):
                yield filename

    counter = 0
    try:
        while True:
            pyfile = (yield)
            for filename in find_files_to_remove(pyfile):
                try:
                    log.debug('removing %s', filename)
                    remove(filename)
                    counter += 1
                except (IOError, OSError), e:
                    log.error('cannot remove %s', filename)
                    log.debug(e)
    except GeneratorExit:
        log.info("removed files: %s", counter)


def main():
    usage = '%prog [-p PACKAGE] [DIR_OR_FILE]'
    parser = optparse.OptionParser(usage, version='%prog 1.0')
    parser.add_option('-v', '--verbose', action='store_true', dest='verbose',
        help='turn verbose more one')
    parser.add_option('-q', '--quiet', action='store_false', dest='verbose',
        default=False, help='be quiet')
    parser.add_option('-p', '--package',
        help='specify Debian package name to clean')

    options, args = parser.parse_args()

    if options.verbose or environ.get('PYCLEAN_DEBUG') == '1':
        log.setLevel(logging.DEBUG)
        log.debug('argv: %s', sys.argv)
        log.debug('options: %s', options)
        log.debug('args: %s', args)
    else:
        log.setLevel(logging.WARNING)

    d = destroyer()
    d.next()  # initialize coroutine

    if not options.package and not args:
        parser.print_usage()
        exit(1)

    if options.package:
        log.info('cleaning package %s', options.package)
        pfiles = dpf.from_package(options.package, extensions=('.py', '.so'))
        pfiles = add_namespace_files(pfiles, options.package, action=False)
        pfiles = set(dpf.filter_out_ext(pfiles, ('.so',)))

    if args:
        log.info('cleaning directories: %s', args)
        files = dpf.from_directory(args, extensions=('.py', '.so'))
        files = add_namespace_files(files, action=False)
        files = set(dpf.filter_out_ext(files, ('.so',)))
        if options.package:
            files = files & pfiles
    else:
        files = pfiles

    for filename in files:
        d.send(filename)

if __name__ == '__main__':
    main()
