This folder contains an example layout of both the server and the client workdir.

If you just want to try fBUFS out, open two CLIs in `client` and `server`.

1. Start the server:

    ```sh
    forgedbackup server start
    ```

2. Start the client:

    ```sh
    forgedbackup client start
    ```

Watch as the `client/data` folder is being backed up securely inside of `server/backups/client1` folder.

On the server CLI, launch
```sh
forgedbackup admin list
```
and check that your fresh backup appears in the list. Then run
```sh
forgedbackup admin decompress client1 0 ./decompressed_backup
```