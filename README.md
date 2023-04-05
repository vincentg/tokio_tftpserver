# tokio_tftpserver
A Rust TFTP Server implemented with Tokio Asynchronous Runtime

On Unix, you will need to specify an user to drop privileges and the base directory

```
Usage: tokio_tftpserver [OPTIONS] --user <USER_TO_DROP_PRIVILEGES_TO> --directory <BASE_DIRECTORY>

Options:
  -b, --bind <BIND>                        [default: 127.0.0.1]
  -p, --port <PORT>                        [default: 69]
  -u, --user <USER_TO_DROP_PRIVILEGES_TO>
  -d, --directory <BASE_DIRECTORY>
  -h, --help
```

On Windows, directory shared will be the current directory
```
Usage: tokio_tftpserver.exe [OPTIONS]

Options:
  -b, --bind <BIND>  [default: 127.0.0.1]
  -p, --port <PORT>  [default: 69]
  -h, --help         Print help
```
