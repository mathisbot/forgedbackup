This folder contains an example layout of both the server and the client workdir.

If you just want to try ForgedBackup out, open two CLIs in `client` and `server`.

1. Start the server:

    ```sh
    forgedbackup server start
    ```

2. Start the client:

    ```sh
    forgedbackup client start
    ```

    Watch as the `client/data` folder is being backed up securely inside of `server/backups/client1` folder.

3. Stop the server with `CTRL+C`

4. On the server CLI, launch
    ```sh
    forgedbackup admin list
    ```
    and check that your fresh backup appears in the list. Then run

5. Decompress the backup
    ```sh
    forgedbackup admin decompress client1 0 ./decompressed_backup
    ```