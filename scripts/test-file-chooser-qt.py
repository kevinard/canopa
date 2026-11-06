#!/usr/bin/env python3
"""Verify ordinary QFileDialog routing to Canopa Iced in a private Niri session."""
import sys
sys.dont_write_bytecode = True
from file_chooser_toolkit import main

if __name__ == '__main__':
    main('qt')
