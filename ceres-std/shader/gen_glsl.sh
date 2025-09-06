#!/bin/bash

~/.cargo/bin/naga gb_screen.wgsl vshader.vert --profile es310 --entry-point vs_main
~/.cargo/bin/naga gb_screen.wgsl fshader.frag --profile es310 --entry-point fs_main