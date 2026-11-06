"""Owned Linux process whose leader exits while one thread still holds stdin."""
import ctypes
import os
import sys
import threading

ready = threading.Event()


def worker():
    ready.set()
    sys.stdin.buffer.read(1)
    os._exit(0)


threading.Thread(target=worker).start()
ready.wait()
print('ready', flush=True)
library = ctypes.CDLL(None)
library.pthread_exit.argtypes = [ctypes.c_void_p]
library.pthread_exit(None)
