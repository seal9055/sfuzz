#!/usr/bin/python3

from libc import *
from common import *

def SIGINT_Handler(sig, frame):
    print("\n[-] Exiting (CTRL-C)")
    sys.exit(0)

def AbsoluteFilePaths(directory):
   
    for dirpath,_,filenames in os.walk(directory):
       for f in filenames:
           yield os.path.abspath(os.path.join(dirpath, f))


def PatchInterpreter(binary, linker):
    patchelf = subprocess.Popen(["patchelf", "--set-interpreter", linker, binary],
            stdout = subprocess.PIPE,
            stderr = subprocess.PIPE)
    patchelf.communicate()
    returncode = patchelf.returncode
    return returncode

def PatchRPath(binary):

    patchelf = subprocess.Popen(["patchelf", "--set-rpath", os.getcwd(), binary],
            stdout = subprocess.PIPE,
            stderr = subprocess.PIPE)
    patchelf.communicate()
    returncode = patchelf.returncode
    return returncode


def PatchFunction(binary, funcname):
    with open(binary, "rb") as f:
        data = f.read()
    
    if funcname in data:
        data = data.replace(funcname, b"abs".ljust(len(funcname), b'\x00'))
        with open(binary, "wb") as f:
            f.write(data)

def GenerateTemplate(binary, host, port):
   
    if host == None:
        host = "127.0.0.1"

    if port == None:
        port = "1337"

    template_path = os.path.join(os.path.dirname(os.path.realpath(__file__)), 'template.py')
    with open(template_path, 'r') as t:
        template = t.read()

    template = template % (binary, host, port)
    template_path = os.path.join(os.getcwd(), 'exploit.py')
    with open(template_path, 'w') as t:
        t.write(template)
        print("[+] Exploit      : 'exploit.py' Template Generated")
        return

def main(binary, libcfile, linkerfile, host, port):

    print("[+] Binary       : %s" % binary)


    cdfiles = AbsoluteFilePaths(os.getcwd())
    libfile = None
    if libcfile == None:
        for file in cdfiles:
            if CheckLibc(file):
                libfile = file
                break

        libc = Libc(libfile)
    
    if libcfile == None and libc.filename == None:
        GenerateTemplate(binary, host, port)
        return 
    
    if not os.path.exists(binary):
        print("[-] Error %s does not exist" % binary)
        exit(-1)


    print("[+] Libc         : %s" % libc.filename) 
    
    if libc.filename != "libc.so.6":
        os.rename(libc.fullpath, os.path.join(os.getcwd(), "libc.so.6"))
        libc.filename = "libc.so.6"
        libc.fullpath = os.path.abspath("./libc.so.6")

    print("[+] Version      : %s" % libc.version)

    libc.GetGLibcPkg()

    if libc.debugfile != None:
        code = libc.Unstrip()
        if code != 0:
            print("[-] Error        : \"eu-unstrip\" [%.d] -- %s" % (code, libc.filename))
        else:
            print("[+] Patched      : %s" % libc.filename)
    
    if linkerfile == None:
        if libc.linker.GetLinker() != True:
            print("[-] Error        : Failed to fetch linker")
        linkerfile = libc.linker.fullpath

    if linkerfile != None:
        linkerfile = os.path.abspath(linkerfile)
        libc.linker.fullpath = linkerfile
        libc.linker.filename = os.path.basename(linkerfile)
        if not os.path.exists(os.path.join(os.getcwd(), libc.linker.filename)):
            shutil.move(libc.linker.fullpath, os.getcwd())

        libc.linker.fullpath = os.path.join(os.getcwd(), libc.linker.filename)

        if PatchInterpreter(binary, libc.linker.fullpath) != 0:
            print("[-] Error        : \"patchelf\" failed to patch interpreter")
    
    if PatchRPath(binary) != 0:
        print("[-] Error        : \"patchelf\" failed to patch rpath")
    
    else:

        PatchFunction(binary, b"alarm")
        print("[+] Patched      : %s" % binary)
    
    
    GenerateTemplate(binary, host, port)


if __name__ == "__main__":

    
    signal.signal(signal.SIGINT, SIGINT_Handler)

    parser = argparse.ArgumentParser(
        description="initiate environment for pwning in CTFs."
        )

    parser.add_argument('-b', '--binary', dest='binary', help='initiate environment for this binary')
    parser.add_argument('-s', '--search', dest='search', action='store_true', help='search libc')
    parser.add_argument('-pr', '--patch-rpath', action='store_true', dest='patch_rpath', help="patch binary's rpath")
    parser.add_argument('-pi', '--patch-interpreter', action='store_true', dest='patch_interpreter', help="patch binary's interpreter")
    parser.add_argument('-lc', '--libc', dest='libc', help='libc binary for patching')
    parser.add_argument('-ld', '--ld', dest='linker', help='linker binary for patching')
    parser.add_argument('-H', '--host', dest='host', help='vulnerable server ip')
    parser.add_argument('-P', '--port', dest='port', help='vulnerable server port')
    parser.add_argument('-t', '--template', action='store_true', dest='template', help='generate template exploit.py')
    args = parser.parse_args()
    
    
    if CheckDependencies() == 0:
        if args.binary and not args.patch_rpath and not args.patch_interpreter and not args.template:
            main(args.binary, args.libc, args.linker, args.host, args.port)
    
        elif args.search:
            Search()
    
        elif args.patch_rpath:
            if args.binary:
                if PatchRPath(args.binary) != 0:
                    print("[-] Error     : \"patchelf\" failed to patch rpath")
                else:
                    print("[+] Patched   : %s" % args.binary)
            else:
                parser.print_usage()
        
        elif args.patch_interpreter:
            if args.binary:
                if args.linker:
                    if PatchInterpreter(args.binary, args.linker) != 0:
                        print("[-] Error     : \"patchelf\" failed to patch interpreter")

                    else:
                        print("[+] Patched   : %s" % args.binary)
                else:
                    parser.print_usage()
            else:
                parser.print_usage()

        elif args.template:
            if args.binary:
                GenerateTemplate(args.binary, args.host, args.port)
            else:
                parser.print_usage()

        else:
            parser.print_usage()
    else:
        exit(-1)
