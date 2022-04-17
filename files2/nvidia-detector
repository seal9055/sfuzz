#! /usr/bin/python3
import NvidiaDetector
from NvidiaDetector.nvidiadetector import NvidiaDetection, NoDatadirError
import sys

if __name__ == '__main__':
    try:
        a = NvidiaDetection(printonly=True, verbose=False)
    except NoDatadirError:
        sys.exit(0)
