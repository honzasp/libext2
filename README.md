# libext2

This is a school project

## Build requirements

You will need to [download and install Rust](rust) and also install `libfuse`
development files (`sudo apt-get install libfuse-dev` on Ubuntu).

## Build the Fuse example

To build the library and the example, please run

    cargo build --example fuse

in the root of the repository. The command will download and build necessary
dependencies from creates.io, the repository of Rust packages, and then it will
compile the library and the example. The compiled executable will be located in
`target/debug/examples/fuse`.

## Run the Fuse example

The example can be executed like this:

    ./target/debug/examples/fuse <ext2-file> <mount-point>

It will use `<ext2-file>` as the volume (I strongly suggest using a regular file
-- *do not use on real volumes containing important data*) and will mount it on
`<mount-point>` (it will fail if the mount point does not exist). The process
will exit once the filesystem is unmounted, either using `umount` or `fusermount
-u`. Killing or terminating the process will not unmount the filesystem.

[rust]: https://www.rust-lang.org/downloads.html
