#!/usr/bin/env python3
"""Optional Rscript-style launcher for isolated BLAS experiments, never runtime QC.

PG_R_BINARY must identify the pinned R bin/exec/R (not its shell wrapper).
PG_BLAS_DIRECTORY overrides dependencies in this child only. macOS protected
shell interpreters can strip inherited DYLD_* settings; set them immediately
before exec and verify the actually loaded libraries with backend_probe.c.
"""
import os
from pathlib import Path
import sys

binary=Path(os.environ['PG_R_BINARY']).resolve()
environment=dict(os.environ)
if environment.get('PG_BLAS_DIRECTORY'):
    directory=Path(environment['PG_BLAS_DIRECTORY']).resolve(strict=True)
    variable='DYLD_LIBRARY_PATH' if sys.platform=='darwin' else 'LD_LIBRARY_PATH'
    environment[variable]=str(directory)
if environment.get('PG_DYLD_PRINT_LIBRARIES')=='1':
    environment['DYLD_PRINT_LIBRARIES']='1'
arguments=sys.argv[1:]
if arguments and arguments[0]=='-e':
    options=arguments
else:
    if not arguments:raise SystemExit('Expected R script path or -e CODE')
    options=['--file='+arguments[0],'--args',*arguments[1:]]
os.execve(binary,[str(binary),'--no-echo','--no-restore',*options],environment)
