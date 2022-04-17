#! /usr/bin/python2

import os, re, sys
try:
    SetType = set
except NameError:
    import sets
    SetType = sets.Set
    set = sets.Set

_defaults = None
def read_default(name=None):
    global _defaults
    from ConfigParser import SafeConfigParser, NoOptionError
    if not _defaults:
        if os.path.exists('/usr/share/python/debian_defaults'):
            config = SafeConfigParser()
            try:
                config.readfp(file('/usr/share/python/debian_defaults'))
            except IOError, msg:
                print msg
                sys.exit(1)
            _defaults = config
    if _defaults and name:
        try:
            value = _defaults.get('DEFAULT', name)
        except NoOptionError:
            raise ValueError
        return value
    return None

def parse_versions(vstring, add_exact=False):
    import operator
    operators = { None: operator.eq, '=': operator.eq,
                  '>=': operator.ge, '<=': operator.le,
                  '<<': operator.lt
                  }
    vinfo = {}
    exact_versions = set([])
    version_range = set(supported_versions(version_only=True)
                        + old_versions(version_only=True))
    relop_seen = False
    for field in vstring.split(','):
        field = field.strip()
        if field == 'all':
            vinfo['all'] = 'all'
            continue
        if field in ('current', 'current_ext'):
            vinfo['current'] = field
            continue
        vinfo.setdefault('versions', set())
        ve = re.compile('(>=|<=|<<|=)? *(\d\.\d)$')
        m = ve.match(field)
        try:
            if not m:
                raise ValueError('error parsing Python-Version attribute')
            op, v = m.group(1), m.group(2)
            vmaj, vmin = v.split('.')
            # Don't silently ignore Python 3 versions.
            if int(vmaj) > 2:
                raise ValueError('error parsing Python-Version attribute, Python 3 version found')
            if op in (None, '='):
                exact_versions.add(v)
            else:
                relop_seen = True
                filtop = operators[op]
                version_range = [av for av in version_range if filtop(av ,v)]
        except Exception:
            raise ValueError, 'error parsing Python-Version attribute'
    if add_exact:
        if exact_versions:
            vinfo['vexact'] = exact_versions
        if 'versions' in vinfo:
            if relop_seen:
                vinfo['versions'] = set(version_range)
            else:
                del vinfo['versions']
    else:
        if 'versions' in vinfo:
            vinfo['versions'] = exact_versions
            if relop_seen:
                vinfo['versions'] = exact_versions.union(version_range)
    return vinfo

_old_versions = None
def old_versions(version_only=False):
    global _old_versions
    if not _old_versions:
        try:
            value = read_default('old-versions')
            _old_versions = [s.strip() for s in value.split(',')]
        except ValueError:
            _old_versions = []
    if version_only:
        return [v[6:] for v in _old_versions]
    else:
        return _old_versions

_unsupported_versions = None
def unsupported_versions(version_only=False):
    global _unsupported_versions
    if not _unsupported_versions:
        try:
            value = read_default('unsupported-versions')
            _unsupported_versions = [s.strip() for s in value.split(',')]
        except ValueError:
            _unsupported_versions = []
    if version_only:
        return [v[6:] for v in _unsupported_versions]
    else:
        return _unsupported_versions

_supported_versions = ["python%s" % ver.strip() for ver in
                       os.environ.get('DEBPYTHON_SUPPORTED', '').split(',')
                       if ver.strip()]
def supported_versions(version_only=False):
    global _supported_versions
    if not _supported_versions:
        try:
            value = read_default('supported-versions')
            _supported_versions = [s.strip() for s in value.split(',')]
        except ValueError:
            cmd = ['/usr/bin/apt-cache', '--no-all-versions',
                   'show', 'python-all']
            try:
                import subprocess
                p = subprocess.Popen(cmd, bufsize=1,
                                     shell=False, stdout=subprocess.PIPE)
                fd = p.stdout
            except ImportError:
                fd = os.popen(' '.join(cmd))
            depends = None
            for line in fd:
                if line.startswith('Depends:'):
                    depends = line.split(':', 1)[1].strip().split(',')
            fd.close()
            if depends:
                depends = [re.sub(r'\s*(\S+)[ (]?.*', r'\1', s) for s in depends]
                _supported_versions = depends
            if not _supported_versions:
                # last resort: python-minimal not installed, apt-cache
                # not available, hard code the value, #394084
                _supported_versions = ['python2.6', 'python2.7']
    if version_only:
        return [v[6:] for v in _supported_versions]
    else:
        return _supported_versions

_default_version = "python%s" % os.environ.get('DEBPYTHON_DEFAULT', '')
if _default_version == 'python':
    _default_version = None
def default_version(version_only=False):
    global _default_version
    if not _default_version:
        try:
            _default_version = link = os.readlink('/usr/bin/python2')
        except OSError:
            _default_version = None
            try:
                cmd = ['/usr/bin/python2', '-c', 'import sys; print sys.version[:3]']
                import subprocess
                p = subprocess.Popen(cmd, bufsize=1,
                                     shell=False, stdout=subprocess.PIPE)
                fd = p.stdout
            except ImportError:
                fd = os.popen("/usr/bin/python2 -c 'import sys; print sys.version[:3]'")
            line = fd.readline().strip()
            fd.close()
            if re.match(r'\d\.\d$', line):
                _default_version = 'python' + line
        # consistency check
        try:
            debian_default = read_default('default-version')
        except ValueError:
            debian_default = "python2.7"
        if not _default_version in (debian_default, os.path.join('/usr/bin', debian_default)):
            raise ValueError, "/usr/bin/python2 does not match the python2 default version. It must be reset to point to %s" % debian_default
        _default_version = debian_default
    if version_only:
        return _default_version[6:]
    else:
        return _default_version

def requested_versions(vstring, version_only=False):
    versions = None
    vinfo = parse_versions(vstring, add_exact=True)
    supported = supported_versions(version_only=True)
    if len(vinfo) == 1:
        if 'all' in vinfo:
            versions = supported
        elif 'current' in vinfo:
            versions = [default_version(version_only=True)]
        elif 'vexact' in vinfo:
            versions = vinfo['vexact']
        else:
            versions = vinfo['versions'].intersection(supported)
    elif 'all' in vinfo and 'current' in vinfo:
        raise ValueError, "both `current' and `all' in version string"
    elif 'all' in vinfo:
        if 'versions' in vinfo:
            versions = vinfo['versions'].intersection(supported)
        else:
            versions = set(supported)
        if 'vexact' in vinfo:
            versions.update(vinfo['vexact'])
    elif 'current' in vinfo:
        current = default_version(version_only=True)
        if not current in vinfo['versions']:
            raise ValueError, "`current' version not in supported versions"
        versions = [current]
    elif 'versions' in vinfo or 'vexact' in vinfo:
        versions = set()
        if 'versions' in vinfo:
            versions = vinfo['versions'].intersection(supported)
        if 'vexact' in vinfo:
            versions.update(vinfo['vexact'])
    else:
        raise ValueError, 'No Python versions in version string'
    if not versions:
        raise ValueError('computed set of supported versions is empty')
    if version_only:
        return versions
    else:
        return ['python%s' % v for v in versions]

def installed_versions(version_only=False):
    import glob
    supported = supported_versions()
    versions = [os.path.basename(s)
                for s in glob.glob('/usr/bin/python[0-9].[0-9]')
                if os.path.basename(s) in supported]
    versions.sort()
    if version_only:
        return [v[6:] for v in versions]
    else:
        return versions

class ControlFileValueError(ValueError):
    pass
class MissingVersionValueError(ValueError):
    pass

def extract_pyversion_attribute(fn, pkg):
    """read the debian/control file, extract the X-Python-Version or
    XS-Python-Version field; check that XB-Python-Version exists for the
    package."""

    version = None
    sversion = None
    section = None
    try:
        fp = file(fn, 'r')
    except IOError, msg:
        print "Cannot open %s: %s" % (fn, msg)
        sys.exit(2)
    for line in fp:
        line = line.strip()
        if line == '':
            if section == None:
                continue
            if pkg == 'Source':
                break
            section = None
        elif line.startswith('Source:'):
            section = 'Source'
        elif line.startswith('Package: ' + pkg):
            section = pkg
        elif line.lower().startswith(('xs-python-version:', 'x-python-version:')):
            if section != 'Source':
                raise ValueError, \
                      'attribute X(S)-Python-Version not in Source section'
            sversion = line.split(':', 1)[1].strip()
        elif line.lower().startswith('xb-python-version:'):
            if section == pkg:
                version = line.split(':', 1)[1].strip()
    if section == None:
        raise ControlFileValueError, 'not a control file'
    if pkg == 'Source':
        if sversion == None:
            raise MissingVersionValueError, \
                  'no X(S)-Python-Version in control file'
        return sversion
    if version == None:
        raise MissingVersionValueError, \
              'no XB-Python-Version for package `%s' % pkg
    return version

# compatibility functions to parse debian/pyversions

def version_cmp(ver1,ver2):
    v1=[int(i) for i in ver1.split('.')]
    v2=[int(i) for i in ver2.split('.')]
    return cmp(v1,v2)

def requested_versions_bis(vstring, version_only=False):
    versions = []
    py_supported_short = supported_versions(version_only=True)
    for item in vstring.split(','):
        v=item.split('-')
        if len(v)>1:
            if not v[0]:
                v[0] = py_supported_short[0]
            if not v[1]:
                v[1] = py_supported_short[-1]
            for ver in py_supported_short:
                try:
                    if version_cmp(ver,v[0]) >= 0 \
                           and version_cmp(ver,v[1]) <= 0:
                        versions.append(ver)
                except ValueError:
                    pass
        else:
            if v[0] in py_supported_short:
                versions.append(v[0])
    versions.sort(version_cmp)
    if not versions:
        raise ValueError, 'empty set of versions'
    if not version_only:
        versions=['python'+i for i in versions]
    return versions

def extract_pyversion_attribute_bis(fn):
    vstring = file(fn).readline().rstrip('\n')
    return vstring

def main():
    from optparse import OptionParser
    usage = '[-v] [-h] [-d|--default] [-s|--supported] [-i|--installed] [-r|--requested <version string>|<control file>]'
    parser = OptionParser(usage=usage)
    parser.add_option('-d', '--default',
                      help='print the default python version',
                      action='store_true', dest='default')
    parser.add_option('-s', '--supported',
                      help='print the supported python versions',
                      action='store_true', dest='supported')
    parser.add_option('-r', '--requested',
                      help='print the python versions requested by a build; the argument is either the name of a control file or the value of the X(S)-Python-Version attribute',
                      action='store_true', dest='requested')
    parser.add_option('-i', '--installed',
                      help='print the installed supported python versions',
                      action='store_true', dest='installed')
    parser.add_option('-v', '--version',
                      help='print just the version number(s)',
                      default=False, action='store_true', dest='version_only')
    opts, args = parser.parse_args()
    program = os.path.basename(sys.argv[0])

    if opts.default and len(args) == 0:
        try:
            print default_version(opts.version_only)
        except ValueError, msg:
            print "%s:" % program, msg
            sys.exit(1)
    elif opts.supported and len(args) == 0:
        print ' '.join(supported_versions(opts.version_only))
    elif opts.installed and len(args) == 0:
        print ' '.join(installed_versions(opts.version_only))
    elif opts.requested and len(args) <= 1:
        if len(args) == 0:
            versions = 'debian/control'
        else:
            versions = args[0]
        try:
            if os.path.isfile(versions):
                fn = versions
                try:
                    vstring = extract_pyversion_attribute(fn, 'Source')
                    vs = requested_versions(vstring, opts.version_only)
                except ControlFileValueError:
                    sys.stderr.write("%s: not a control file: %s, " \
                                     % (program, fn))
                    sys.exit(1)
                except MissingVersionValueError:
                    fn = os.path.join(os.path.dirname(fn), 'pyversions')
                    sys.stderr.write("%s: missing X(S)-Python-Version in control file, fall back to %s\n" \
                                     % (program, fn))
                    try:
                        vstring = extract_pyversion_attribute_bis(fn)
                        vs = requested_versions_bis(vstring, opts.version_only)
                    except IOError:
                        sys.stderr.write("%s: missing debian/pyversions file, fall back to supported versions\n" \
                                         % program)
                        vs = supported_versions(opts.version_only)
                except ValueError, e:
                    sys.stderr.write("%s: %s\n" % (program, e))
                    sys.exit(4)
            else:
                vs = requested_versions(versions, opts.version_only)
            print ' '.join(vs)
        except ValueError, msg:
            sys.stderr.write("%s: %s\n" % (program, msg))
            sys.exit(1)
    else:
        sys.stderr.write("usage: %s %s\n" % (program, usage))
        sys.exit(1)

if __name__ == '__main__':
    main()
