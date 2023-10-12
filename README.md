<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->
**Table of Contents**  *generated with [DocToc](https://github.com/thlorenz/doctoc)*

- [KeePass4Web](#keepass4web)
  - [FEATURES](#features)
  - [INSTALL](#install)
  - [BUILD FRONTEND](#build-frontend)
  - [CONFIGURATION](#configuration)
  - [DEPLOYMENT](#deployment)
    - [Container](#container)
    - [Classic](#classic)
  - [BACKENDS](#backends)
    - [Authentication](#authentication)
        - [LDAP](#ldap)
    - [Database](#database)
        - [Filesystem](#filesystem)
  - [MISC](#misc)
  - [LIMITATIONS](#limitations)
  - [APP DETAILS / BACKGROUND](#app-details--background)
    - [Sequence of client/server operations](#sequence-of-clientserver-operations)
  - [COPYRIGHT AND LICENSING](#copyright-and-licensing)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

# KeePass4Web

A mobile-friendly web application that serves KeePass database on a web frontend.

Written in Perl and JavaScript.


## FEATURES

- Doesn't save master password/keyfile, uses a new and unique encryption key to cache the database
- Encryption key is stored securely in the kernel keyring
- Server revokes encryption keys after a configurable user idle time, effectively removing access to the cached database
- Web interface offers entry search and access to files stored inside the database. Also displays custom entry icons


![Login](doc/img/login.png)

![App](doc/img/app.png)


## INSTALL

- From container image:
See [DEPLOYMENT](#deployment)

- From source:
    - Clone the repo to some dir
        > git clone https://github.com/lixmal/keepass4web-rs.git

        > cd keepass4web-rs

    - Follow [BUILD FRONTEND](#build-frontend), [DEPLOYMENT](#deployment) in that order


## BUILD FRONTEND

The minified, bundled file will be written to public/scripts/bundle.js

- Install Node/npm, e.g. for Ubuntu
    > sudo apt-get install npm

- Install js modules
    > npm install

- Copy bootstrap font files
    > cp node_modules/bootstrap/fonts/* public/fonts/

- Build js bundle
    > npm run build

- For a non-uglified version you can run
    > npm run dev


## CONFIGURATION

- See `config.yml`


## DEPLOYMENT

### Container

See [GitHub Packages](https://ghcr.io/lixmal/keepass4web-rs)

The image ships with the default config in `/conf/config.yml`, which should be overwritten with a mount/volume.

The app makes use of the [Linux kernel keyring](https://man7.org/linux/man-pages/man7/keyrings.7.html).

The keyring is currently not namespaced, hence container tooling deactivate the specific syscalls by default.
To make the app run you will need to activate the syscalls by creating a custom seccomp profile and passing the path to the container runtime:

- [Docker](https://docs.docker.com/engine/security/seccomp/)
- [podman](https://docs.podman.io/en/v4.6.0/markdown/options/seccomp-policy.html)

A base file for extension can be found [here](https://github.com/moby/moby/blob/master/profiles/seccomp/default.json), see the `syscalls` section.

The required syscalls are:

- keyctl
- add_key
- request_key

### Classic

This requires rust installed, compile the binary:

    export RUSTFLAGS="-Ctarget-cpu=sandybridge -Ctarget-feature=+aes,+sse2,+sse4.1,+ssse3"
    cargo build --bins --release --target-dir release

Run the binary:

    target/release/keepass4web-rs


**Make sure no other containers are running under the same user, or they will be able to access keys stored for keepass4web**.

This is best achieved by running rootless containers with a dedicated user for keepass4web.

- [Docker](https://docs.docker.com/engine/security/rootless/)
- [podman](https://github.com/containers/podman/blob/main/docs/tutorials/rootless_tutorial.md)


## BACKENDS

### Authentication

##### LDAP

Attempts to authenticate the user against an (external, not built-in) LDAP server (Microsoft AD, 389 Directory Server, OpenLDAP, ...)

### Database

##### Filesystem

Grabs the KeePass database from the local filesystem.
Can get database and key file location from auth backend.
Web server needs read access to the files.

## MISC

- Show kernel keyrings in use (as root)
    > sudo cat /proc/keys

    > sudo cat /proc/key-users


## LIMITATIONS

- KeePass databases are read-only
- Limits of kernel keyring apply



## APP DETAILS / BACKGROUND
### Sequence of client/server operations

```
Client                                                       Server


Load website /
                              request KeePass tree
                              -------------------->

                                                        Check sesssion

                              not authenticated
                              <--------------------

Redirect to /user_login
Show credentials dialog

                              user credentials
                              -------------------->

                                                        User auth (LDAP, SQL, ...)

                                           login OK
                              <--------------------

Redirect to /backend_login
Show backend login dialog

                              backend credentials
                              -------------------->

                                                        Init DB backend / receive backend token
                                           login OK
                              <--------------------

Redirect to /db_login
Show KeePass password dialog

                              KeePass credentials
                              -------------------->
                                                        Get KeePass database from backend
                                                        Decrypt KeePass database with master key + key file
                                                        Encrypt serialised string with newly generated key
                                                        Put encryption key into kernel keyring
                                                        Write keyring key id to session
                                                        Place encrypted database into cache
                                      decryption OK
                              <--------------------

Redirect to /
                              request KeePass tree
                              -------------------->

                                                        Get database from cach
                                                        Get keyring key id from session
                                                        Get encryption key from kernel keyring
                                                        Decrypt database with key

                                  Send KeePass tree
                              <--------------------
Show KeePass tree

...

Password request by user
                              Request pw entry
                              -------------------->

                                                        Get keyring key id from session
                                                        Get encryption key from kernel keyring
                                                        Get database from cache
                                                        Decrypt database
                                                        Decrypt requested password

                                  Send pw entry
                              <--------------------
Show cleartext pw

```

## COPYRIGHT AND LICENSING

This software is copyright (c) by Viktor Liu.
It is released under the terms of the GPL version 3.

Most of the icons in the `public/img/icons` directory are released under the LGPL version 2, the licence can be found in the same directory.
The remaining icons are public domain.
As these icons are the same as the ones used by the original KeePass software, you can refer to the info there: [Icon Acknowledgements](http://keepass.info/help/base/credits.html#icons).
