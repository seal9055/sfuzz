#! /usr/bin/python2

from os.path import exists
from subprocess import call
from sys import argv
from re import compile

OLD = '/usr/share/python/dh_python2'
NEW = '/usr/share/dh-python/dh_python2'
has_dhpython = compile(r'(^|:|\s|,)dh-python($|\s|,|\()').search

binary = OLD
if exists(NEW) and exists('debian/control'):
    with open('debian/control', 'r') as fp:
        inside = False
        for line in fp:
            if not line:
                break
            line_lower = line.lower()
            if inside:
                if line.startswith((' ', "\t")):
                    if has_dhpython(line):
                        binary = NEW
                        break
                    continue
                elif line.startswith('#'):
                    continue
                inside = False
            if line_lower.startswith(('build-depends:', 'build-depends-indep:')):
                if has_dhpython(line):
                    binary = NEW
                    break
                inside = True

argv[0] = binary
exit(call(argv))
