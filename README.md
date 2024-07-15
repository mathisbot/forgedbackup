# ForgedBackup

ForgedBackup is a tool written in Rust for creating and automating fast, secure backups.

[![Contributors][contributors-shield]][contributors-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]

## Architecture

The architecture pays particular attention to optimization and safety.

Let's call "client" the server that wants to be backed up and "server" the server that actually hosts backups.

It's up to the client to decide when to initiate a backup.
When it does:
1. It authenticates to the server, and the server authenticates itself to the client.
This is done so that your private data cannot be sent anywhere else than on the chosen servers, and that you don't accept data incoming from unknown servers.
2. An encrypted pipe is opened, making sure that no one can eavesdrop on your private data.
3. The client sends the files to be backed up to the server
4. The server compresseses them on the fly

The code is made out of four major modules :

### Asynchronous Directory Crawler (fADC)

fADC is a tool that provides an asynchronous API for crawling through directories.
It provides a stream containing the content of the directory similarly to what tarball does.
It is useful as backups contains multiple files that will be stored as one.

### Compression Engine (fCE)

fCE is a compression utility based on the LZ4 algorithm. It combines speed and compression ratio, and is particularly well-suited to real-time compression.
Decompression is also very fast, making it easy for users to browse through backups.

I may update to use the latest LZ4r algorithm for a better compression ratio if needed.

### Data General Security Engine (fDGSE)

fDGSE is a traffic encryption tool that uses the AES256-GCM system to guarantee both the integrity and confidentiality of transmitted data.

### Server Authentication System (fSAS)

fSAS is a utility for securely authenticating a server when it connects, with the aim of receiving backups only from selected servers, and sending backups only to selected endpoints.

## Performances

fBUFS takes advantage of the capabilities of modern processors, using multiple CPU cores to maximize performance.

The majority of costly tasks are outsourced to the server, so as not to impact the normal operation of the source server. In particular, I have chosen to compress data after it has been sent, so that the client is only required to encrypt the data. It was therefore assumed that the connection between the two servers was not the limiting factor.

In practice, the bottleneck is encryption/decryption.

## Getting started

### Prerequisites

* Rust/Cargo

### Installation

* Clone the project :

    ```sh
    git clone https://github.com/mathisbot/forgedbackup.git
    ```

* Build the project

    ```sh
    cargo build --release
    ```

    Binary file will be located in `target/release/forgedbackup`

### Usage

First, make sure clients and servers are running the same version of fBUFS.

Head over `example` if you want a quick example layout for both the client and the server workdir. Please do not copy/paste it in production because it contains private keys.

Each time you want to pair a client and a server, you will have to perform these operations :

1. Initialize the client:
    ```sh
    forgedbackup client init [dest_dir]
    ```

    This will generate authentication keys.

    Place the private key and the AES key in `<WORKDIR>/signing_key/<server_name>` and `<WORKDIR>/cipher_keys/<server_name>` respectively.

    You will also have to send the public key as well as the AES key to the server in `<WORKDIR>/verifying_keys/<client_name>` and `<WORKDIR>/cipher_keys/<client_name>` respectively.

2. Initialize the server:
    ```sh
    forgedbackup server init [dest_dir]
    ```

    This will generate authentication keys.

    Place the private key in `<WORKDIR>/signing_key/<client_name>`

    You will also have to send the public key to the client in `<WORKDIR>/verifying_keys/<server_name>`.

3. Configure both machines

    Create a file named `<WORKDIR>/config.toml`. This is where the 
    configuration is stored. 
    
    You can configure the location of the directories mentionned above as well as other parameters.

    Server configuration must contains these keys:

    ```toml
    listening_on="127.0.0.1:8080"
    signing_keys_dir="signing_keys"
    verifying_keys_dir="verifying_keys"
    cipher_keys_dir="cipher_keys"
    backup_dir="backups_dir"
    ```

    Client configuration must contains these keys:

    ```toml
    signing_keys_dir="signing_keys"
    verifying_keys_dir="verifying_keys"
    cipher_keys_dir="cipher_keys"
    hostname="client1"
    backed_up_dir="data"

    [servers]
    server1 = "127.0.0.1:8080"
    # ...
    ```

4. Run fBUFS

    On the server :
    ```sh
    forgedbackup server start
    ```

    This will make the server listen on the specified address.
    He will now answer to incoming connection.

    On the client :
    ```sh
    forgedbackup client start
    ```

    The client will successively attempt to perform a backup on each of the specified backup servers.

5. List backup (on the server) :

    ```sh
    forgedbackup admin list
    ```

6. Decompress a backup (on the same server) :

    ```sh
    forgedbackup admin decompress <client> <backup-number> [output-dir]
    ```

### Linux service

It is important to ensure that fBUFS is always ready to receive backups on the backup server. For this reason, its is recommended to create a service managed by systemd.
To do so, you can follow these steps:

1. Create a new file at `/etc/systemd/system/forgedbackup.service` containing:

    ```plaintext
    [Unit]
    Description=fBUFS
    After=network.target

    [Service]
    ExecStart=/path/to/forgedbackup server start
    WorkingDirectory=/path/to/forgedbackup
    User=username
    Group=groupname
    Restart=always

    [Install]
    WantedBy=multi-user.target
    ```

    Replace `/path/to/forgedbackup` with the actual path to the `forgedbackup` binary file and replace `username` and `groupname` with a Linux username and group name that can at least read files to be backed up.

4. Reload the systemd daemon to load the new service unit:

    ```sh
    sudo systemctl daemon-reload
    ```

5. Enable the fBUFS service to start on boot:

    ```sh
    sudo systemctl enable forgedbackup.service
    ```

6. Start the fBUFS service:

    ```sh
    sudo systemctl start forgedbackup.service
    ```

7. Verify that the service is running:

    ```sh
    sudo systemctl status forgedbackup.service
    ```

    You should see the status of the service and any associated logs.

## Important Notes

Currently, backups are not rotated. Even if compression does a great job, this means that memory can be filled up and backups made impossible.

You should add monitoring over free space on your server and manually delete old backups once in a while.

## LICENSE

This project is licensed under the GNU GENERAL PUBLIC LICENSE.
This license allows everyone to participate in the project while prohibiting closed-source.


[contributors-shield]: https://img.shields.io/github/contributors/mathisbot/forgedbackup.svg?style=for-the-badge
[contributors-url]: https://github.com/mathisbot/forgedbackup/graphs/contributors
[stars-shield]: https://img.shields.io/github/stars/mathisbot/forgedbackup.svg?style=for-the-badge
[stars-url]: https://github.com/mathisbot/forgedbackup/stargazers
[issues-shield]: https://img.shields.io/github/issues/mathisbot/forgedbackup.svg?style=for-the-badge
[issues-url]: https://github.com/mathisbot/forgedbackup/issues