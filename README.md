# Voxotrace

## Overview

Voxotrace renders ultra detailed scenes using a fully raytraced pipline in realtime.

As hardware gets more powerful, and geometry starts to only take up a few pixels on the screen, why bother with geometry at all?   
Enter Voxotrace, rendering sub pixel detail with unconstrained transparency, with blazingly fast framerates.
Voxotrace uses sparse voxel oct-DAGs to unify geometry, materials, and a raytracing acceleration structure into a single system.


## Setup 

!!! Currently only works with DX12, which is not hard coded so it may try to select something else.
Hope to fix this when switching to newer wgpu version.

Requires Rust 1.31 or later.

